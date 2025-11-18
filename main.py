#!/usr/bin/env python3
from scraper import Scraper

scraper = Scraper()
lines = ["karsiyaka-esenyurt", "polis-loj-toki", "gultepe", "4-eylul-san-toptancilar"]
station = 10
buses = scraper.get_live_buses_for_station(station, lines)
print(buses)