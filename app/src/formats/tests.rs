use super::*;
fn bits(values: &[(u64, usize)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut bit = 0;
    for &(value, width) in values {
        for i in (0..width).rev() {
            if bit % 8 == 0 {
                bytes.push(0);
            }
            let n = bytes.len() - 1;
            bytes[n] |= (((value >> i) & 1) as u8) << (7 - bit % 8);
            bit += 1;
        }
    }
    bytes
}
fn set_len(bytes: &mut [u8], len: usize) {
    bytes[..3].copy_from_slice(&[(len >> 16) as u8, (len >> 8) as u8, len as u8]);
}
fn bufr_message(desc: &[u32], count: u16, compressed: bool, values: &[(u64, usize)]) -> Vec<u8> {
    let mut s1 = vec![0; 22];
    set_len(&mut s1, 22);
    s1[5] = 7;
    s1[15..17].copy_from_slice(&2026u16.to_be_bytes());
    s1[17] = 11;
    s1[18] = 7;
    let mut s3 = vec![0; 7];
    s3[4..6].copy_from_slice(&count.to_be_bytes());
    s3[6] = if compressed { 64 } else { 0 };
    for &d in desc {
        s3.push(((d / 100000) << 6 | ((d / 1000) % 100)) as u8);
        s3.push((d % 1000) as u8);
    }
    let len = s3.len();
    set_len(&mut s3, len);
    let mut s4 = vec![0; 4];
    s4.extend(bits(values));
    let len = s4.len();
    set_len(&mut s4, len);
    let mut bytes = b"BUFR\0\0\0\x04".to_vec();
    bytes.extend(s1);
    bytes.extend(s3);
    bytes.extend(s4);
    bytes.extend(b"7777");
    let len = bytes.len();
    set_len(&mut bytes[4..], len);
    bytes
}
#[test]
fn bufr_uncompressed_geolocation_and_temperature() {
    let data = bufr_message(
        &[5001, 6001, 12001],
        2,
        false,
        &[
            (12_800_000, 25),
            (9_400_000, 26),
            (2931, 12),
            (12_900_000, 25),
            (9_500_000, 26),
            (2941, 12),
        ],
    );
    let messages = bufr::decode_all(&data);
    let m = &messages[0];
    assert_eq!(m.error, "");
    assert_eq!(m.subsets.len(), 2);
    assert_eq!(bufr::lat_lon(&m.subsets[0]), Some((38., -86.)));
    assert_eq!(bufr::lat_lon(&m.subsets[1]), Some((39., -85.)));
    assert!((m.subsets[0][2].value.unwrap() - 293.1).abs() < 1e-6);
    assert_eq!(bufr::main_element(m).unwrap().0, 12001);
}
#[test]
fn bufr_compressed_reference_and_increments() {
    let data = bufr_message(
        &[5001, 6001, 12001],
        2,
        true,
        &[
            (12_800_000, 25),
            (17, 6),
            (0, 17),
            (100_000, 17),
            (9_400_000, 26),
            (17, 6),
            (0, 17),
            (100_000, 17),
            (2931, 12),
            (0, 6),
        ],
    );
    let messages = bufr::decode_all(&data);
    let m = &messages[0];
    assert_eq!(m.error, "");
    assert_eq!(bufr::lat_lon(&m.subsets[1]), Some((39., -85.)));
    assert!((m.subsets[1][2].value.unwrap() - 293.1).abs() < 1e-6);
}
#[test]
fn bufr_delayed_replication_and_truncated_input() {
    let data = bufr_message(
        &[101000, 31001, 12001],
        1,
        false,
        &[(3, 8), (2800, 12), (2810, 12), (2820, 12)],
    );
    let m = bufr::decode_all(&data).remove(0);
    assert_eq!(m.error, "");
    assert_eq!(m.subsets[0].len(), 4);
    for len in 0..data.len() {
        let messages = bufr::decode_all(&data[..len]);
        assert!(messages.iter().all(|m| !m.error.is_empty()));
    }
    let bad = bufr_message(&[203015], 1, false, &[]);
    let m = bufr::decode_all(&bad).remove(0);
    assert!(!m.error.is_empty());
    assert!(m.subsets.is_empty());
}
#[test]
fn bilinear_preserves_a_plane_and_edges() {
    let p = Points {
        name: "plane".into(),
        units: String::new(),
        values: vec![(0., 0., 0.), (0., 1., 10.), (1., 0., 100.), (1., 1., 110.)],
    };
    let g = regrid(&p, (0., 0., 1., 1.), 3, 3, "bilinear");
    assert_eq!(g.values, vec![100., 105., 110., 50., 55., 60., 0., 5., 10.]);
}
#[test]
fn nearest_finds_distant_sparse_points() {
    let p = Points {
        values: vec![(0., 0., 42.)],
        ..Default::default()
    };
    let g = regrid(&p, (-5., -5., 5., 5.), 3, 3, "nearest");
    assert!(g.values.iter().all(|v| *v == 42.));
}
#[test]
fn pipeline_converts_units_preserves_missing_and_rejects_typos() {
    let mut g = Grid2D {
        bbox: (0., 0., 1., 1.),
        nx: 2,
        ny: 2,
        values: vec![-40., 0., 100., f32::NAN],
    };
    let before = g.values.clone();
    assert!(apply_pipeline_checked(&mut g, &["scale typo".into()]).is_err());
    assert_eq!(&g.values[..3], &before[..3]);
    apply_pipeline_checked(&mut g, &["c_to_f".into(), "clip -40 212".into()]).unwrap();
    assert_eq!(&g.values[..3], &[-40., 32., 212.]);
    assert!(g.values[3].is_nan());
    assert_eq!(output_units("K", &["k_to_f".into()]), "°F");
}

/// Minimal classic NetCDF fixture with a non-terminal spatial dimension: [lat,time,lon].
/// Verifies the real reader and stride handling, rather than reproducing the decoder.
fn netcdf_fixture() -> Vec<u8> {
    fn word(b: &mut Vec<u8>, v: u32) {
        b.extend(v.to_be_bytes());
    }
    fn name(b: &mut Vec<u8>, s: &str) {
        word(b, s.len() as u32);
        b.extend(s.as_bytes());
        while b.len() % 4 != 0 {
            b.push(0);
        }
    }
    let mut b = b"CDF\x01".to_vec();
    word(&mut b, 0);
    word(&mut b, 10);
    word(&mut b, 3);
    for (n, size) in [("lat", 2), ("time", 2), ("lon", 2)] {
        name(&mut b, n);
        word(&mut b, size);
    }
    word(&mut b, 0);
    word(&mut b, 0);
    word(&mut b, 11);
    word(&mut b, 3);
    let mut positions = Vec::new();
    for (n, dims, size) in [
        ("lat", vec![0], 16),
        ("lon", vec![2], 16),
        ("temperature", vec![0, 1, 2], 64),
    ] {
        name(&mut b, n);
        word(&mut b, dims.len() as u32);
        for d in dims {
            word(&mut b, d);
        }
        word(&mut b, 0);
        word(&mut b, 0);
        word(&mut b, 6);
        word(&mut b, size);
        positions.push(b.len());
        word(&mut b, 0);
    }
    for (pos, values) in positions.into_iter().zip([
        vec![38., 39.],
        vec![-87., -86.],
        vec![10., 11., 90., 91., 20., 21., 92., 93.],
    ]) {
        let offset = b.len() as u32;
        b[pos..pos + 4].copy_from_slice(&offset.to_be_bytes());
        for v in values {
            b.extend(f64::to_be_bytes(v));
        }
    }
    b
}
#[test]
fn netcdf_actual_reader_honors_spatial_strides() {
    let path = std::env::temp_dir().join(format!("cwx-{}.nc", uuid::Uuid::new_v4()));
    std::fs::write(&path, netcdf_fixture()).unwrap();
    let result = (|| {
        assert_eq!(detect(&path), Format::NetCdf3);
        let inv = inspect(&path).unwrap();
        assert_eq!(inv.variables.len(), 3);
        decode(&path, 2, None).unwrap()
    })();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        result.values,
        vec![
            (38., -87., 10.),
            (38., -86., 11.),
            (39., -87., 20.),
            (39., -86., 21.)
        ]
    );
}

#[test]
fn grib2_simple_packing_decodes_geographic_points() {
    fn section(number: u8, len: usize) -> Vec<u8> {
        let mut b = vec![0; len];
        b[..4].copy_from_slice(&(len as u32).to_be_bytes());
        b[4] = number;
        b
    }
    let mut s1 = section(1, 21);
    s1[5..7].copy_from_slice(&7u16.to_be_bytes());
    s1[9] = 2;
    s1[11] = 1;
    s1[12..14].copy_from_slice(&2026u16.to_be_bytes());
    s1[14] = 11;
    s1[15] = 7;
    let mut s3 = section(3, 72);
    s3[6..10].copy_from_slice(&4u32.to_be_bytes());
    s3[14] = 6;
    s3[15..30].fill(255);
    for (offset, value) in [
        (30, 2u32),
        (34, 2),
        (46, 38_000_000),
        (50, 273_000_000),
        (55, 39_000_000),
        (59, 274_000_000),
        (63, 1_000_000),
        (67, 1_000_000),
    ] {
        s3[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    s3[42..46].fill(255);
    s3[54] = 48;
    s3[71] = 64;
    let mut s4 = section(4, 34);
    s4[11] = 2;
    s4[17] = 1;
    s4[22] = 1;
    s4[28..34].fill(255);
    let mut s5 = section(5, 21);
    s5[5..9].copy_from_slice(&4u32.to_be_bytes());
    s5[11..15].copy_from_slice(&300f32.to_be_bytes());
    let mut s6 = section(6, 6);
    s6[5] = 255;
    let s7 = section(7, 5);
    let mut b = b"GRIB\0\0\0\x02\0\0\0\0\0\0\0\0".to_vec();
    for s in [s1, s3, s4, s5, s6, s7] {
        b.extend(s);
    }
    b.extend(b"7777");
    let len = b.len() as u64;
    b[8..16].copy_from_slice(&len.to_be_bytes());
    let path = std::env::temp_dir().join(format!("cwx-{}.grb2", uuid::Uuid::new_v4()));
    std::fs::write(&path, b).unwrap();
    let result = decode(&path, 0, None);
    let _ = std::fs::remove_file(path);
    let p = result.unwrap();
    assert_eq!(p.values.len(), 4);
    assert!(p.values.iter().all(|(lat, lon, v)| *lat >= 38.
        && *lat <= 39.
        && *lon >= -87.
        && *lon <= -86.
        && *v == 300.));
}

#[test]
fn hdf5_compressed_field_uses_coordinates_and_fill_value() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/formats/fixtures/rectilinear.h5");
    let inv = inspect(&path).unwrap();
    let index = inv
        .variables
        .iter()
        .find(|v| v.name.ends_with("temperature"))
        .unwrap()
        .index;
    let p = decode(&path, index, None).unwrap();
    assert_eq!(p.units, "degC");
    assert_eq!(
        p.values,
        vec![(38., -87., 10.), (38., -86., 11.), (39., -87., 20.)]
    );
}

#[test]
fn netcdf4_applies_packing_and_missing_values() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/formats/fixtures/packed.nc");
    assert_eq!(detect(&path), Format::NetCdf4);
    let inv = inspect(&path).unwrap();
    let index = inv
        .variables
        .iter()
        .find(|v| v.name == "temperature")
        .unwrap()
        .index;
    let p = decode(&path, index, None).unwrap();
    assert_eq!(
        p.values,
        vec![(38., -87., 10.), (38., -86., 12.), (39., -87., 14.)]
    );
}
