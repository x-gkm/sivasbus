#!/usr/bin/env python3
from scraper import Scraper
import time

import pandas as pd

scraper = Scraper()
bus_datas = []
for line in lines:
    bus_data = scraper.get_live_buses(line)
    bus_datas.append(bus_data)
    time.sleep(0.2)

buses = pd.concat(bus_datas)
station = scraper.get_live_station(10)

print(buses)
print(station)