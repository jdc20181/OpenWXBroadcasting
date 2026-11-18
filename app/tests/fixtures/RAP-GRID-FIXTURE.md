# RAP map-grid fixture (approved-relay contract)

File: `rap-grid-indiana-20261115t12z-f01.grib2` (69,144 bytes, 15 GRIB2 messages).

Captured 2026-11-15 by Claude for the Codex request "provide one actual approved-relay
`model-frame?source=rap&fields=grid` GRIB2 response for a small Indiana region".

## How it was made

The relay's `/model-frame` route returns the upstream NOMADS body unchanged
(`sharedFetch(modelUrl(req.query))`). The upstream URL below was produced by calling the
relay's own exported `modelUrl()` (DCWeatherSite `src/routes/weatherbroadcaster.js`) with:

```
source=rap fields=grid run=1794744000 (2026-11-15 12Z) lead=1
south=37.7 north=41.8 west=-88.1 east=-84.7   (relay snaps to 35..45 N, -90..-80)
```

```
https://nomads.ncep.noaa.gov/cgi-bin/filter_rap.pl?subregion=&leftlon=-90&rightlon=-80&toplat=45&bottomlat=35&dir=%2Frap.20261115&file=rap.t12z.awp130pgrbf01.grib2&lev_2_m_above_ground=on&lev_10_m_above_ground=on&lev_surface=on&lev_mean_sea_level=on&lev_entire_atmosphere=on&var_TMP=on&var_DPT=on&var_RH=on&var_UGRD=on&var_VGRD=on&var_GUST=on&var_MSLMA=on&var_TCDC=on&var_APCP=on&var_CAPE=on&var_CRAIN=on&var_CSNOW=on&var_CFRZR=on&var_CICEP=on
```

HTTP 200, `application/octet-stream`. The deployed relay was not used (it has not been
redeployed yet); the bytes are identical to what it would relay for this query.

## Verified contents

- Centre 7 (NCEP), reference time 2026-11-15T12:00Z, significance 1 (start of forecast).
- Grid template 3.30 (Lambert conformal, AWIPS 130 subset), 71 × 90 points.
- Product template 4.0, lead 1 hour, except APCP: template 4.8 (accumulation 0–1 h).

| # | Discipline/cat/num | Field | Level |
|---|---|---|---|
| 1 | 0/2/22 | GUST | surface |
| 2 | 0/3/198 | MSLMA | mean sea level |
| 3 | 0/0/0 | TMP | surface |
| 4 | 0/0/0 | TMP | 2 m |
| 5 | 0/0/6 | DPT | 2 m |
| 6 | 0/1/1 | RH | 2 m |
| 7 | 0/2/2 | UGRD | 10 m |
| 8 | 0/2/3 | VGRD | 10 m |
| 9 | 0/1/8 | APCP | surface, 0–1 h acc (PDT 4.8) |
| 10–13 | 0/1/195, 194, 193, 192 | CSNOW, CICEP, CFRZR, CRAIN | surface |
| 14 | 0/7/6 | CAPE | surface |
| 15 | 0/6/1 | TCDC | entire atmosphere (type 10) |

Note: GUST arrives at the surface level, not 10 m; TCDC at "entire atmosphere";
there is no 2 m dew point under any other level. U/V are grid-relative (resolution flags
0b00111000, bit 5 set) on a Lambert grid with LaD 25° and LoV 265° (−95°): rotate before
computing direction. Units are GRIB2 defaults (K, Pa,
m/s, %, kg/m², J/kg).
