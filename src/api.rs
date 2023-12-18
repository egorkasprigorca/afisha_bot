use std::collections::HashMap;

use serde::{Deserialize, Serialize};

const AFISHA_API_ROOT: &str = "https://afisha.yandex.ru/api/";

pub const CATEGORIES: [&str; 7] = [
    "cinema", "concert", "theatre", "art", "standup", "show", "quest",
];

#[derive(Serialize, Deserialize, Debug)]
pub struct Resp {
    data: Vec<Elements>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Resp2 {
    paging: Smth,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Smth {
    total: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Elements {
    event: Event,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    pub id: String,
    pub url: String,
    pub title: String
}

pub async fn get_events(city: String, categories: Vec<String>, period: u32) -> Vec<Event> {
    // let resp = reqwest::get(format!(
    //     "{}{}{}{}{}{}{}{}{}{}",
    //     AFISHA_API_ROOT,
    //     "events/actual?",
    //     "city=",
    //     city,
    //     "&tag=",
    //     categories[0],
    //     "&period=",
    //     period,
    //     "&date=",
    //     chrono::Local::now()
    //         .to_string()
    //         .split(" ")
    //         .collect::<Vec<_>>()[0]
    // ))
    // .await
    // .unwrap();
    // println!("{}", resp.status());
    // let json = resp.json::<Resp2>().await.unwrap();
    // let mut total = json.paging.total;
    // let mut offset = 0;
    // let step = 12;
    // let mut events = Vec::new();
    // while total > 0 {
    //     let resp = reqwest::get(format!(
    //         "{}{}{}{}{}{}{}{}{}{}",
    //         AFISHA_API_ROOT,
    //         "events/actual?",
    //         "city=",
    //         city,
    //         "&tag=",
    //         categories[0],
    //         "&period=",
    //         period,
    //         "&date=",
    //         chrono::Local::now()
    //             .to_string()
    //             .split(" ")
    //             .collect::<Vec<_>>()[0]
    //     ))
    //     .await
    //     .unwrap();
    //     let json = resp.json::<Resp>().await.unwrap();
    //     offset += step;
    //     total -= step;
    //     for event in json.data {
    //         events.push(event.event);
    //     }
    // }
    let resp = reqwest::get(format!(
        "{}{}{}{}{}{}{}{}",
        AFISHA_API_ROOT,
        "events/actual?",
        "period=",
        period,
        "&city=",
        city,
        "&tag=",
        categories[0]
    ))
    .await
    .unwrap();
    println!("{}", resp.status());
    let json = resp.json::<Resp2>().await.unwrap();
    let mut total = json.paging.total;
    let mut offset = 0;
    let step = 12;
    let mut events = Vec::new();
    while total > 0 {
        let resp = reqwest::get(format!(
            "{}{}{}{}{}{}{}{}",
            AFISHA_API_ROOT,
            "events/actual?",
            "city=",
            city,
            "&tag=",
            categories[0],
            "&period=",
            period
        ))
        .await
        .unwrap();
        let json = resp.json::<Resp>().await.unwrap();
        offset += step;
        total -= step;
        for event in json.data {
            events.push(event.event);
        }
    }
    events
}

pub async fn test_api() {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    let resp = client
        .get("https://api.afisha.yandex.ru/cities")
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Accept", "application/json")
        .header("Host", "https://api.afisha.yandex.ru")
        .send()
        .await
        .unwrap();
    // let json = resp.json::<Resp>().await.unwrap();
    // json
    println!("{:#?}", resp);
}
