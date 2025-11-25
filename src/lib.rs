use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

const SITE_ROOT: &str = "https://ulasim.sivas.bel.tr";

#[derive(Error, Debug)]
pub enum Error {
    #[error("request error")]
    Request(#[from] reqwest::Error),
    #[error("can't deserialize json")]
    Json(#[from] serde_json::Error),
    #[error("error parsing station")]
    StationError(#[from] StationError),
    #[error("error parsing a bus from a line")]
    LineBusError(#[from] LineBusError),
    #[error("token not found")]
    NoToken,
    #[error("stations not found")]
    NoStations,
    #[error("line id not found")]
    NoLineId,
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Coords {
    pub lat: f64,
    pub long: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LineBus {
    pub license_plate: String,
    pub coords: Coords,
}

#[derive(Serialize, Deserialize, Debug)]
struct LineBusDto {
    #[serde(rename = "aracPlaka")]
    license_plate: String,
    #[serde(rename = "mevcutlat")]
    lat: String,
    #[serde(rename = "mevcutlng")]
    long: String,
}

#[derive(Error, Debug)]
#[error("can't parse the bus data from a line")]
pub struct LineBusError(#[from] std::num::ParseFloatError);

impl TryFrom<LineBusDto> for LineBus {
    type Error = LineBusError;

    fn try_from(dto: LineBusDto) -> Result<LineBus, LineBusError> {
        Ok(LineBus {
            license_plate: dto.license_plate.trim().to_string(),
            coords: Coords {
                lat: dto.lat.parse()?,
                long: dto.long.parse()?,
            },
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StationBus {
    pub license_plate: String,
    pub arrive_time: Duration,
}

#[derive(Serialize, Deserialize, Debug)]
struct StationBusDto {
    #[serde(rename = "plaka")]
    license_plate: String,
    #[serde(rename = "sure")]
    arrive_time_secs: u64,
}

impl From<StationBusDto> for StationBus {
    fn from(dto: StationBusDto) -> StationBus {
        StationBus {
            license_plate: dto.license_plate.trim().to_string(),
            arrive_time: Duration::from_secs(dto.arrive_time_secs),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Station {
    pub id: i32,
    pub human_name: String,
    pub coords: Coords,
}

#[derive(Serialize, Deserialize, Debug)]
struct StationDto {
    #[serde(rename = "linko")]
    path: String,
    #[serde(rename = "durakAd")]
    human_name: String,
    #[serde(rename = "durakLat")]
    lat: String,
    #[serde(rename = "durakLng")]
    long: String,
}

#[derive(Error, Debug)]
pub enum StationError {
    #[error("can't find the id of the station")]
    NoId,
    #[error("can't parse the id of the station")]
    Id(#[from] std::num::ParseIntError),
    #[error("can't parse station coordinates")]
    Coords(#[from] std::num::ParseFloatError),
}

impl TryFrom<StationDto> for Station {
    type Error = StationError;

    fn try_from(dto: StationDto) -> Result<Station, StationError> {
        Ok(Station {
            id: dto
                .path
                .split("/")
                .last()
                .ok_or(StationError::NoId)?
                .parse()?,
            human_name: dto.human_name.trim().to_string(),
            coords: Coords {
                lat: dto.lat.parse()?,
                long: dto.long.parse()?,
            },
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Line {
    pub id: String,
    pub human_name: String,
}

pub struct Client(reqwest::Client);

impl Client {
    pub fn new() -> Client {
        Client(
            reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .unwrap(),
        )
    }

    async fn get_document(&self, path: String) -> Result<String, reqwest::Error> {
        let result = self.0
            .get(format!("{SITE_ROOT}{path}"))
            .send()
            .await?
            .text()
            .await?;

        tokio::time::sleep(Duration::from_millis(200)).await;

        Ok(result)
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        params: Vec<(&str, &str)>,
    ) -> Result<T, reqwest::Error> {
        let result = self.0
            .post(format!("{SITE_ROOT}{path}"))
            .form(&params)
            .send()
            .await?
            .json()
            .await?;

        tokio::time::sleep(Duration::from_millis(200)).await;

        Ok(result)
    }

    pub async fn get_lines(&self) -> Result<Vec<Line>> {
        let doc = self.get_document("/".to_string()).await?;
        Ok(extract_lines(&doc))
    }

    pub async fn get_all_stations(&self) -> Result<Vec<Station>> {
        let doc = self
            .get_document("/Akilli-Duraklar-Harita".to_string())
            .await?;
        extract_stations(&doc)
    }

    pub async fn get_stations(&self, line: &str) -> Result<Vec<Station>> {
        let doc = self.get_document(format!("/hat/{line}")).await?;
        extract_stations(&doc)
    }

    pub async fn get_line_buses(&self, line: &str) -> Result<Vec<LineBus>> {
        let doc = self.get_document(format!("/hat/{line}")).await?;
        let token = extract_token(&doc).ok_or(Error::NoToken)?;
        let id = extract_line_id(&doc).ok_or(Error::NoLineId)?;
        let dtos: Vec<LineBusDto> = self
            .post_json(
                "/aractekrar",
                vec![("hgID", id), ("__RequestVerificationToken", &token)],
            )
            .await?;

        let mut results = vec![];
        for dto in dtos {
            results.push(dto.try_into()?);
        }
        Ok(results)
    }

    pub async fn get_station_buses(&self, station: i32) -> Result<Vec<StationBus>> {
        let doc = self
            .get_document(format!("/Akilli-Durak/{station}"))
            .await?;
        let token = extract_token(&doc).ok_or(Error::NoToken)?;
        let json: Vec<StationBusDto> = self
            .post_json(
                "/durakTekrar",
                vec![
                    ("drkID", &station.to_string()),
                    ("__RequestVerificationToken", &token),
                ],
            )
            .await?;

        let results = json.into_iter().map(|dto| dto.into()).collect();

        Ok(results)
    }
}

fn extract_token(doc: &str) -> Option<String> {
    let selector = Selector::parse(r#"input[name="__RequestVerificationToken"]"#).unwrap();
    let html = Html::parse_document(doc);
    let elem = html.select(&selector).next()?;
    let token = elem.attr("value")?;

    Some(token.to_string())
}

fn extract_line_id(doc: &str) -> Option<&str> {
    Some(
        Regex::new(r"hgID\s*:\s*(\d+)")
            .unwrap()
            .captures(&doc)?
            .get(1)?
            .as_str(),
    )
}

fn extract_lines(doc: &str) -> Vec<Line> {
    Html::parse_document(doc)
        .select(&Selector::parse(r#"a[href^="/hat/"]"#).unwrap())
        .filter_map(|elem| {
            let id = elem.attr("href")?.split("/").last()?.to_string();
            let human_name = elem.text().next()?.trim().to_string();
            Some(Line { id, human_name })
        })
        .collect()
}

fn extract_station_json(doc: &str) -> Option<&str> {
    Some(
        Regex::new(r"var\s+duraks\s*=\s*(\[.*\])")
            .unwrap()
            .captures(doc)?
            .get(1)?
            .as_str(),
    )
}

fn extract_stations(doc: &str) -> Result<Vec<Station>> {
    let json = extract_station_json(doc).ok_or(Error::NoStations)?;
    let dtos = serde_json::from_str::<Vec<StationDto>>(json)?;

    let mut results = vec![];
    for dto in dtos {
        results.push(dto.try_into()?);
    }
    Ok(results)
}
