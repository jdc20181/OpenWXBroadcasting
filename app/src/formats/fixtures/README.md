# Generated parser fixtures

Created by tools/generate-format-fixtures.py using h5py 3.16.0 and h5netcdf 1.8.2. These are synthetic 2x2 datasets authored for this project, with no external weather data.

- rectilinear.h5: gzip-compressed float32 Celsius data, latitude/longitude coordinates, -9999 missing cell.
- packed.nc: NetCDF4/HDF5, packed latitude/longitude coordinates and gzip-compressed int16, scale_factor=2, add_offset=10, -9999 missing cell.

Python packages are needed only to regenerate the fixtures. Rust tests read the committed fixtures directly.
