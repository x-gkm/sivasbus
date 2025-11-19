import re
import io
import time

import requests
from bs4 import BeautifulSoup
import pandas as pd

SITE_ROOT = "https://ulasim.sivas.bel.tr"

def extract_token(doc: str):
    soup = BeautifulSoup(doc, "html5lib")
    tag = soup.select_one('input[name="__RequestVerificationToken"]')
    value = tag["value"]
    return value

class Scraper:
    def __init__(self):
        self.session = requests.Session()

    def get_lines(self):
        response = self.session.get(SITE_ROOT)
        time.sleep(0.2)
        soup = BeautifulSoup(response.text, "html5lib")

        tags = soup.select('a[href^="/hat/"]')
        names = []
        human_names = []
        counts = []
        for tag in tags:
            names.append(tag["href"].split("/")[-1])
            human_names.append(" ".join(tag.text.strip().split()[:-1]))
            counts.append(int(tag.find("span").text))
        
        return pd.DataFrame({ "name": names, "human_name": human_names, "count": counts }).sort_values(by=["count"], ascending=False)

    def get_live_buses(self, lines: str | list[str]):
        if isinstance(lines, str):
            lines = [lines]
        
        result = []
        for line in lines:
            result.append(self._get_single_live_line(line))

        return pd.concat(result)

    def _get_single_live_line(self, line: str):
        response = self.session.get(f"{SITE_ROOT}/hat/{line}")
        time.sleep(0.2)
        doc = response.text

        id = int(re.search(r"hgID: (\d+)", doc).group(1))
        token = extract_token(doc)

        payload = {"hgID": id, "__RequestVerificationToken": token}

        response = self.session.post(f"{SITE_ROOT}/aractekrar", data=payload)
        time.sleep(0.2)
        df = pd.read_json(io.BytesIO(response.content))
        if df.empty: return df
        return df \
            .rename(columns={
                "aracPlaka": "license_plate",
                "mevcutlat": "lat",
                "mevcutlng": "long",
                "hatAd": "route",
            }) \
            .loc[:, ["license_plate", "lat", "long", "route"]] \
            .set_index("license_plate")
    
    def get_stations(self):
        response = self.session.get(f"{SITE_ROOT}/Akilli-Duraklar-Liste")
        time.sleep(0.2)
        soup = BeautifulSoup(response.text, "html5lib")

        tags = soup.select('a[href^="/Akilli-Durak/"]')

        ids = []
        for tag in tags:
            path = tag["href"]
            id = path.split("/")[-1]
            ids.append(id)

        names = []
        for tag in tags:
            td = tag.parent.find_previous_sibling("td")
            names.append(td.text)

        return pd.DataFrame({ "name": names }, index = ids)

    def get_live_station(self, station):
        response = self.session.get(f"{SITE_ROOT}/Akilli-Durak/{station}")
        time.sleep(0.2)
        token = extract_token(response.text)

        payload = {"drkID": station, "__RequestVerificationToken": token}

        response = self.session.post(f"{SITE_ROOT}/durakTekrar", data=payload)
        time.sleep(0.2)
        df = pd.read_json(io.BytesIO(response.content))
        if df.empty: return df
        return df \
            .rename(columns={
                "plaka": "license_plate",
                "hatAd": "route",
                "hatkod": "route_code",
                "durakID": "station",
                "sure": "arrive_time_minutes",
            }) \
            .loc[:, ["license_plate", "route", "route_code", "station", "arrive_time_minutes"]] \
            .set_index("license_plate")
    
    def get_live_buses_for_stations(self, station_ids: int | list[int], lines: str | list[str]):
        if isinstance(station_ids, int):
            station_ids = [station_ids]

        return pd.concat(self._get_live_buses_for_single_station(station_id, lines) for station_id in station_ids)

    def _get_live_buses_for_single_station(self, station_id, lines: str | list[str]):
        station = self.get_live_station(station_id)
        if station.empty: return pd.DataFrame()

        buses = self.get_live_buses(lines)
        if buses.empty: return pd.DataFrame()

        buses.drop(columns=["route"], inplace=True)

        return station.merge(buses, on = "license_plate").sort_values(by=["arrive_time_minutes"])
