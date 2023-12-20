use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

use crate::{
    api::get_events,
    db::{get_all_users, init_db, DB_URL},
};
use api::CATEGORIES;
use calendar_duration::CalendarDuration;
use chrono::{DateTime, Local, NaiveTime, Utc};
use db::{get_user, insert_user, update_user, User, UserFilter};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
use teloxide::{
    dispatching::{
        dialogue::{self, InMemStorage},
        UpdateHandler,
    },
    prelude::*,
    utils::{command::BotCommands, html, markdown}, types::{InlineKeyboardMarkup, KeyboardMarkup, KeyboardButton},
};
use tokio::time;

mod api;
mod db;

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "Начало работы с ботом.")]
    Start,
    #[command(description = "Вывод списка всех команд.")]
    Help,
    #[command(description = "Редактирование параметров.")]
    Edit {
        parameter: String,
    },
    #[command(description = "Посмотреть параметры пользователя.")]
    Info
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Default)]
enum State {
    #[default]
    Start,
    City,
    Categories {
        city: String,
    },
    NotificationTime {
        city: String,
        categories: Vec<String>,
    },
    EventsInterval {
        city: String,
        categories: Vec<String>,
        notification_time: NaiveTime,
    },
    EditCity,
    EditCategories,
    EditNotificationTime,
    EditEventsInterval,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting throw dice bot...");

    let bot = Bot::from_env();

    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => {}
            Err(error) => {
                panic!("{}", error)
            }
        }
    }

    let pool = SqlitePool::connect(DB_URL).await.unwrap();
    init_db(&pool).await;

    let timers = tokio::task::spawn({
        let bot = bot.clone();
        let pool = pool.clone();

        async move {
            let mut interval = time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                // let users = USERS.lock().unwrap().clone();
                let users = get_all_users(&pool).await.unwrap();
                let now = Local::now().time();
                for user in users {
                    let diff = (now - user.notification_time).num_minutes();
                    if diff == 0 && now > user.notification_time {
                        let events = get_events(user.city, user.tags, user.events_interval).await;
                        let mut total = events.len();
                        let mut offset = 0;
                        let step = 10;
                        while total > 0 {
                            let mut output = String::new();
                            for event in &events[offset..offset + step] {
                                output = format!(
                                    "{output}\n{}\nhttps://afisha.yandex.ru/{}/n",
                                    event.title, event.url
                                );
                            }
                            bot.send_message(ChatId(user.tg_id.try_into().unwrap()), output)
                                .await
                                .unwrap();
                            offset += step;
                            total -= step;
                        }
                    }
                }
            }
        }
    });

    Dispatcher::builder(bot, schema())
        .dependencies(dptree::deps![InMemStorage::<State>::new(), pool])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
    timers.await.unwrap();
}

fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler = teloxide::filter_command::<Command, _>().branch(
        case![State::Start]
            .branch(case![Command::Help].endpoint(cmd_help))
            .branch(case![Command::Start].endpoint(cmd_start))
            .branch(case![Command::Info].endpoint(cmd_info))
            .branch(case![Command::Edit { parameter }].endpoint(cmd_edit)),
    );
    let message_handler = Update::filter_message()
        .branch(command_handler)
        .branch(case![State::City].endpoint(receive_city))
        .branch(case![State::Categories { city }].endpoint(receive_categories))
        .branch(
            case![State::NotificationTime { city, categories }].endpoint(receive_notification_time),
        )
        .branch(
            case![State::EventsInterval {
                city,
                categories,
                notification_time
            }]
            .endpoint(receive_events_interval),
        )
        .branch(case![State::EditCity].endpoint(receive_edit_city))
        .branch(case![State::EditCategories].endpoint(receive_edit_categories))
        .branch(case![State::EditNotificationTime].endpoint(receive_edit_notification_time))
        .branch(case![State::EditEventsInterval].endpoint(receive_edit_events_interval));
    dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(message_handler)
}

async fn cmd_cancel(bot: Bot, msg: Message, dialogue: MyDialogue) -> HandlerResult {
    dialogue.update(State::Start).await?;
    Ok(())
}

async fn cmd_help(bot: Bot, msg: Message) -> HandlerResult {
    bot.send_message(msg.chat.id, Command::descriptions().to_string())
        .await?;
    Ok(())
}

async fn cmd_info(bot: Bot, msg: Message, pool: SqlitePool) -> HandlerResult {
    let user = get_user(&pool, msg.chat.id.0 as u64).await.unwrap();
    let tg_id = user.tg_id;
    let city = user.city;
    let categories = user.tags;
    let categories_to_print = categories.join(" ");
    let notification_time = user.notification_time;
    let events_interval = user.events_interval;
    bot.send_message(
        msg.chat.id,
        format!(
            "Вы выбрали\nВаше id: {tg_id}\nВаш город: {city}\nКатегории: {categories_to_print}\nВремя оповещений: {notification_time}\nИнтервал предстоящих событий: {events_interval}"
        ),
    )
    .await?;
    Ok(())
}

async fn cmd_start(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message
) -> HandlerResult {
    bot.send_message(msg.chat.id, "Давайте начнем! Из какого вы города?")
        .await?;
    dialogue.update(State::City).await?;
    Ok(())
}

const PARAMETERS: [&str; 4] = [
    "city",
    "categories",
    "notification_time",
    "events_intervall",
];

async fn cmd_edit(
    bot: Bot,
    msg: Message,
    parameter: String,
    dialogue: MyDialogue,
) -> HandlerResult {
    match parameter.as_str() {
        "city" => {
            bot.send_message(msg.chat.id, "Введите новый город").await?;
            dialogue.update(State::EditCity).await?;
        }
        "categories" => {
            bot.send_message(msg.chat.id, "Введите новые категории")
                .await?;
            dialogue.update(State::EditCategories).await?;
        }
        "notification_time" => {
            bot.send_message(msg.chat.id, "Введите новое время для уведомлений")
                .await?;
            dialogue.update(State::EditNotificationTime).await?;
        }
        "events_interval" => {
            bot.send_message(msg.chat.id, "Введите новые интервалы")
                .await?;
            dialogue.update(State::EditEventsInterval).await?;
        }
        _ => {
            bot.send_message(msg.chat.id, "Неправильный параметр")
                .await?;
        }
    }
    Ok(())
}

async fn receive_edit_city(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot, msg, dialogue).await?;
                return Ok(());
            }
            update_user(
                &pool,
                UserFilter {
                    id: None,
                    tg_id: None,
                    city: Some(text.into()),
                    tags: None,
                    notification_time: None,
                    events_interval: None,
                },
                msg.chat.id.0 as u64     
            )
            .await
            .unwrap();
            dialogue.exit().await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправьте ваш город.")
                .await?;
        }
    }
    Ok(())
}

async fn receive_edit_categories(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot, msg, dialogue).await?;
                return Ok(());
            }
            let parts = text.split(',');
            let mut categories = Vec::new();
            for part in parts {
                match CATEGORIES.iter().find(|el| **el == part.trim()) {
                    Some(_text) => {}
                    None => {
                        bot.send_message(msg.chat.id, format!("{part} не категория."))
                            .await?;
                        dialogue
                            .update(State::EditCategories)
                            .await?;
                        return Ok(());
                    }
                };
                categories.push(part.to_string());
            }
            update_user(
                &pool,
                UserFilter {
                    id: None,
                    tg_id: None,
                    city: None,
                    tags: Some(categories),
                    notification_time: None,
                    events_interval: None,
                },
                msg.chat.id.0 as u64     
            )
            .await
            .unwrap();
            dialogue.exit().await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправьте ваш город.")
                .await?;
        }
    }
    Ok(())
}

async fn receive_edit_notification_time(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot, msg, dialogue).await?;
                return Ok(());
            }
            let parts: Vec<&str> = text.split(':').collect();
            let hour = match parts[0].parse::<u32>() {
                Ok(num) => num,
                Err(_) => {
                    dialogue.update(State::EditNotificationTime).await?;
                    return Ok(())
                },
            };
            let min = match parts[1].parse::<u32>() {
                Ok(num) => num,
                Err(_) => {
                    dialogue.update(State::EditNotificationTime).await?;
                    return Ok(())
                },
            };
            let notification_time = NaiveTime::from_hms_opt(
                hour,
                min,
                0,
            )
            .unwrap();
            update_user(
                &pool,
                UserFilter {
                    id: None,
                    tg_id: None,
                    city: None,
                    tags: None,
                    notification_time: Some(notification_time),
                    events_interval: None,
                },
                msg.chat.id.0 as u64     
            )
            .await
            .unwrap();
            dialogue.exit().await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправьте ваш город.")
                .await?;
        }
    }
    Ok(())
}

async fn receive_edit_events_interval(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot, msg, dialogue).await?;
                return Ok(());
            }
            let events_interval = text.parse::<u32>().unwrap();
            update_user(
                &pool,
                UserFilter {
                    id: None,
                    tg_id: None,
                    city: None,
                    tags: None,
                    notification_time: None,
                    events_interval: Some(events_interval),
                },
                msg.chat.id.0 as u64 
            )
            .await
            .unwrap();
            dialogue.exit().await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправьте ваш город.")
                .await?;
        }
    }
    Ok(())
}

async fn receive_city(bot: Bot, dialogue: MyDialogue, msg: Message) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot.clone(), msg.clone(), dialogue.clone()).await?;
                return Ok(());
            }
            bot.send_message(msg.chat.id, format!("Выберите категории событий."))
                .await?;
            dialogue
                .update(State::Categories { city: text.into() })
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправьте ваш город.")
                .await?;
        }
    }
    Ok(())
}

async fn receive_categories(
    bot: Bot,
    dialogue: MyDialogue,
    city: String,
    msg: Message,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot.clone(), msg.clone(), dialogue.clone()).await?;
                return Ok(());
            }
            let parts = text.split(',');
            let mut categories = Vec::new();
            for part in parts {
                match CATEGORIES.iter().find(|el| **el == part.trim()) {
                    Some(_text) => {}
                    None => {
                        bot.send_message(msg.chat.id, format!("{part} не категория."))
                            .await?;
                        dialogue
                            .update(State::Categories { city: city.clone() })
                            .await?;
                        return Ok(());
                    }
                };
                categories.push(part.to_string());
            }
            bot.send_message(
                msg.chat.id,
                format!("Выберите время оповещения. Пример: 22:10:57"),
            )
            .await?;
            dialogue
                .update(State::NotificationTime {
                    city: city,
                    categories: categories,
                })
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Отправьте категории.")
                .await?;
        }
    }

    Ok(())
}

async fn receive_notification_time(
    bot: Bot,
    dialogue: MyDialogue,
    (city, categories): (String, Vec<String>),
    msg: Message,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot.clone(), msg.clone(), dialogue.clone()).await?;
                return Ok(());
            }
            bot.send_message(
                msg.chat.id,
                format!("Выберите интервал для предстоящих событий в днях."),
            )
            .await?;
            //input example 22:10
            let parts: Vec<&str> = text.split(':').collect();
            let hour = match parts[0].parse::<u32>() {
                Ok(num) => num,
                Err(_) => {
                    dialogue.update(State::EditNotificationTime).await?;
                    return Ok(())
                },
            };
            let min = match parts[1].parse::<u32>() {
                Ok(num) => num,
                Err(_) => {
                    dialogue.update(State::EditNotificationTime).await?;
                    return Ok(())
                },
            };
            let naive_time = NaiveTime::from_hms_opt(
                hour,
                min,
                0,
            )
            .unwrap();
            dialogue
                .update(State::EventsInterval {
                    city,
                    categories,
                    notification_time: naive_time,
                })
                .await?;
        }
        None => {
            bot.send_message(msg.chat.id, "Выберите интервал.").await?;
        }
    }

    Ok(())
}

async fn receive_events_interval(
    bot: Bot,
    dialogue: MyDialogue,
    (city, categories, notification_time): (String, Vec<String>, NaiveTime),
    msg: Message,
    pool: SqlitePool,
) -> HandlerResult {
    match msg.text() {
        Some(text) => {
            if text == "/cancel" {
                cmd_cancel(bot.clone(), msg.clone(), dialogue.clone()).await?;
                return Ok(());
            }
            let tg_id = msg.from().unwrap().id.0;
            let categories_to_print = categories.join(" ");
            let events_interval = text.parse::<u32>().unwrap();
            bot.send_message(
                msg.chat.id,
                format!(
                    "Вы выбрали\nВаше id: {tg_id}\nВаш город: {city}\nКатегории: {categories_to_print}\nВремя оповещений: {notification_time}\nИнтервал предстоящих событий: {events_interval}"
                ),
            )
            .await?;
            dialogue.exit().await?;

            let user = User {
                id: -1,
                tg_id: tg_id,
                city: city,
                tags: categories,
                notification_time: notification_time,
                events_interval: events_interval,
            };
            insert_user(&pool, user.clone()).await;
        }
        None => {
            bot.send_message(msg.chat.id, "Send me categories.").await?;
        }
    }

    Ok(())
}
