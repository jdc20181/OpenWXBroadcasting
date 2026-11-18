//! Bounded BUFR 3/4 decoder. WMO Table B/D, replication and common operators.
//! Unsupported local tables/operators fail the message instead of producing misaligned data.
use anyhow::{anyhow, bail, Result};
use std::{collections::HashMap, sync::OnceLock};

#[derive(Clone, Debug)]
pub struct Value {
    pub fxy: u32,
    pub value: Option<f64>,
}
#[derive(Clone, Debug, Default)]
pub struct Message {
    pub edition: u8,
    pub centre: u16,
    pub category: u8,
    pub time: String,
    pub subsets: Vec<Vec<Value>>,
    pub error: String,
}
#[derive(Clone)]
struct Element {
    name: String,
    units: String,
    scale: i32,
    reference: i64,
    width: usize,
}
struct Tables {
    b: HashMap<u32, Element>,
    d: HashMap<u32, Vec<u32>>,
}
fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let mut b = HashMap::new();
        let mut d: HashMap<u32, Vec<u32>> = HashMap::new();
        for line in include_str!("bufr_table_b.tsv").lines() {
            let p: Vec<_> = line.split('\t').collect();
            if p.len() == 6 {
                if let (Ok(id), Ok(scale), Ok(reference), Ok(width)) =
                    (p[0].parse(), p[3].parse(), p[4].parse(), p[5].parse())
                {
                    b.insert(
                        id,
                        Element {
                            name: p[1].into(),
                            units: p[2].into(),
                            scale,
                            reference,
                            width,
                        },
                    );
                }
            }
        }
        for line in include_str!("bufr_table_d.tsv").lines() {
            let p: Vec<_> = line.split('\t').collect();
            if p.len() == 2 {
                if let (Ok(id), Ok(child)) = (p[0].parse(), p[1].parse()) {
                    d.entry(id).or_default().push(child);
                }
            }
        }
        Tables { b, d }
    })
}
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}
impl Bits<'_> {
    fn read(&mut self, width: usize) -> Result<u64> {
        if width > 63
            || self
                .pos
                .checked_add(width)
                .filter(|n| *n <= self.data.len() * 8)
                .is_none()
        {
            bail!("truncated BUFR data or unsupported bit width {width}");
        }
        let mut v = 0;
        for _ in 0..width {
            v = (v << 1) | ((self.data[self.pos / 8] >> (7 - self.pos % 8)) & 1) as u64;
            self.pos += 1;
        }
        Ok(v)
    }
    fn skip(&mut self, width: usize) -> Result<()> {
        if self
            .pos
            .checked_add(width)
            .filter(|n| *n <= self.data.len() * 8)
            .is_none()
        {
            bail!("truncated BUFR text");
        }
        self.pos += width;
        Ok(())
    }
}
#[derive(Default)]
struct Ops {
    width: i32,
    scale: i32,
    associated: usize,
    next_width: Option<usize>,
    string_width: Option<usize>,
    combined: i32,
}
struct Decoder<'a> {
    bits: Bits<'a>,
    compressed: bool,
    count: usize,
    emitted: usize,
}
impl Decoder<'_> {
    fn element(
        &mut self,
        id: u32,
        ops: &mut Ops,
        output: &mut [Vec<Value>],
    ) -> Result<Vec<Option<f64>>> {
        let e = tables().b.get(&id).ok_or_else(|| {
            anyhow!("unknown/local descriptor {id:06}; matching provider tables required")
        })?;
        let text = e.units.contains("IA5");
        let width = ops.next_width.take().unwrap_or_else(|| {
            if text {
                ops.string_width.unwrap_or(e.width)
            } else if id / 1000 == 31 {
                e.width
            } else {
                (e.width as i32
                    + ops.width
                    + ((10f64.powi(ops.combined) - 1.).log2().ceil().max(0.) as i32))
                    .max(0) as usize
            }
        });
        if ops.associated > 0 && id / 1000 != 31 {
            if self.compressed {
                self.bits.read(ops.associated)?;
                let n = self.bits.read(6)? as usize;
                for _ in 0..self.count {
                    self.bits.read(n)?;
                }
            } else {
                self.bits.skip(ops.associated)?;
            }
        }
        if text {
            if self.compressed {
                self.bits.skip(width)?;
                let chars = self.bits.read(6)? as usize;
                if chars > 0 {
                    self.bits.skip(chars * 8 * self.count)?;
                }
            } else {
                self.bits.skip(width)?;
            }
            return Ok(vec![None; self.count]);
        }
        let base = self.bits.read(width)?;
        let missing = if width == 0 {
            false
        } else {
            base == (1u64 << width) - 1 && id / 1000 != 31
        };
        let increments = if self.compressed {
            self.bits.read(6)? as usize
        } else {
            0
        };
        let mut values = Vec::with_capacity(self.count);
        for subset in output.iter_mut() {
            let inc = if increments > 0 {
                Some(self.bits.read(increments)?)
            } else {
                None
            };
            let absent = missing || inc.map(|v| v == (1u64 << increments) - 1).unwrap_or(false);
            let special =
                id / 1000 == 31 || e.units.contains("CODE TABLE") || e.units.contains("FLAG TABLE");
            let scale = if special {
                e.scale
            } else {
                e.scale + ops.scale + ops.combined
            };
            let reference = if special {
                e.reference as f64
            } else {
                e.reference as f64 * 10f64.powi(ops.combined)
            };
            let value = (!absent)
                .then(|| (base as f64 + inc.unwrap_or(0) as f64 + reference) * 10f64.powi(-scale));
            subset.push(Value { fxy: id, value });
            values.push(value);
            self.emitted += 1;
            if self.emitted > 2_000_000 {
                bail!("BUFR decoded-value limit exceeded");
            }
        }
        Ok(values)
    }
    fn walk(
        &mut self,
        desc: &[u32],
        ops: &mut Ops,
        output: &mut [Vec<Value>],
        depth: usize,
    ) -> Result<()> {
        if depth > 32 {
            bail!("BUFR descriptor nesting limit exceeded");
        }
        let mut i = 0;
        while i < desc.len() {
            let id = desc[i];
            i += 1;
            let (f, x, y) = (id / 100000, (id / 1000) % 100, id % 1000);
            match f {
                0 => {
                    self.element(id, ops, output)?;
                }
                1 => {
                    let repeats = if y == 0 {
                        let factor = *desc
                            .get(i)
                            .ok_or_else(|| anyhow!("missing replication factor"))?;
                        i += 1;
                        if ![31000, 31001, 31002].contains(&factor) {
                            bail!("unsupported replication factor {factor:06}");
                        }
                        let v = self.element(factor, ops, output)?;
                        let n = v[0].ok_or_else(|| anyhow!("missing replication count"))? as usize;
                        if v.iter().any(|v| *v != Some(n as f64)) {
                            bail!("compressed replication counts differ");
                        }
                        n
                    } else {
                        y as usize
                    };
                    if repeats > 65535 || x == 0 {
                        bail!("invalid BUFR replication count");
                    }
                    let end = i
                        .checked_add(x as usize)
                        .filter(|end| *end <= desc.len())
                        .ok_or_else(|| anyhow!("truncated replication descriptors"))?;
                    for _ in 0..repeats {
                        self.walk(&desc[i..end], ops, output, depth + 1)?;
                    }
                    i = end;
                }
                2 => match x {
                    1 => ops.width = if y == 0 { 0 } else { y as i32 - 128 },
                    2 => ops.scale = if y == 0 { 0 } else { y as i32 - 128 },
                    4 => ops.associated = y as usize,
                    5 => {
                        if self.compressed {
                            bail!("compressed operator text unsupported");
                        }
                        self.bits.skip(y as usize * 8)?;
                    }
                    6 => ops.next_width = Some(y as usize),
                    7 => ops.combined = y as i32,
                    8 => ops.string_width = if y == 0 { None } else { Some(y as usize * 8) },
                    _ => bail!("unsupported BUFR operator {id:06}; no partial data imported"),
                },
                3 => {
                    let sequence = tables()
                        .d
                        .get(&id)
                        .ok_or_else(|| anyhow!("unknown/local sequence {id:06}"))?;
                    self.walk(sequence, ops, output, depth + 1)?;
                }
                _ => bail!("invalid descriptor {id:06}"),
            }
        }
        Ok(())
    }
}
fn length(b: &[u8]) -> Result<usize> {
    if b.len() < 3 {
        bail!("truncated BUFR section");
    }
    Ok((b[0] as usize) << 16 | (b[1] as usize) << 8 | b[2] as usize)
}
fn section<'a>(b: &'a [u8], pos: &mut usize) -> Result<&'a [u8]> {
    let len = length(b.get(*pos..).ok_or_else(|| anyhow!("truncated section"))?)?;
    if len < 4 {
        bail!("invalid BUFR section length");
    }
    let end = pos
        .checked_add(len)
        .filter(|n| *n <= b.len() - 4)
        .ok_or_else(|| anyhow!("section exceeds BUFR message"))?;
    let s = &b[*pos..end];
    *pos = end;
    Ok(s)
}
fn decode(b: &[u8], m: &mut Message) -> Result<()> {
    if b.len() < 12 || &b[b.len() - 4..] != b"7777" {
        bail!("truncated BUFR message/end marker");
    }
    m.edition = b[7];
    if ![3, 4].contains(&m.edition) {
        bail!("BUFR edition {} unsupported", m.edition);
    }
    let mut pos = 8;
    let s1 = section(b, &mut pos)?;
    let optional = if m.edition == 4 {
        if s1.len() < 22 {
            bail!("short BUFR identification section");
        }
        m.centre = u16::from_be_bytes([s1[4], s1[5]]);
        m.category = s1[10];
        m.time = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}Z",
            u16::from_be_bytes([s1[15], s1[16]]),
            s1[17],
            s1[18],
            s1[19],
            s1[20]
        );
        s1[9] & 128 != 0
    } else {
        if s1.len() < 17 {
            bail!("short BUFR identification section");
        }
        m.centre = s1[5] as u16;
        m.category = s1[8];
        m.time = format!(
            "year {:02}, {:02}-{:02} {:02}:{:02}Z",
            s1[12], s1[13], s1[14], s1[15], s1[16]
        );
        s1[7] & 128 != 0
    };
    if optional {
        section(b, &mut pos)?;
    }
    let s3 = section(b, &mut pos)?;
    if s3.len() < 7 || (s3.len() - 7) % 2 != 0 {
        bail!("invalid BUFR descriptor section");
    }
    let count = u16::from_be_bytes([s3[4], s3[5]]) as usize;
    if count > 20000 {
        bail!("BUFR subset limit exceeded");
    }
    let desc: Vec<u32> = s3[7..]
        .chunks_exact(2)
        .map(|c| (c[0] as u32 >> 6) * 100000 + ((c[0] & 63) as u32) * 1000 + c[1] as u32)
        .collect();
    let compressed = s3[6] & 64 != 0;
    let s4 = section(b, &mut pos)?;
    let mut dec = Decoder {
        bits: Bits {
            data: &s4[4..],
            pos: 0,
        },
        compressed,
        count: if compressed { count } else { 1 },
        emitted: 0,
    };
    let mut output = vec![Vec::new(); count];
    if compressed {
        if count > 0 {
            dec.walk(&desc, &mut Ops::default(), &mut output, 0)?;
        }
    } else {
        for subset in &mut output {
            dec.walk(&desc, &mut Ops::default(), std::slice::from_mut(subset), 0)?;
        }
    }
    m.subsets = output;
    Ok(())
}
pub fn decode_all(bytes: &[u8]) -> Vec<Message> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 8 <= bytes.len() && out.len() < 2000 {
        if &bytes[pos..pos + 4] != b"BUFR" {
            pos += 1;
            continue;
        }
        let len = length(&bytes[pos + 4..]).unwrap_or(0);
        let mut m = Message::default();
        let result = if len < 12 || len > bytes.len() - pos {
            Err(anyhow!("truncated/invalid BUFR message length"))
        } else {
            decode(&bytes[pos..pos + len], &mut m)
        };
        if let Err(e) = result {
            m.error = e.to_string();
            m.subsets.clear();
        }
        out.push(m);
        if len < 12 || len > bytes.len() - pos {
            break;
        }
        pos += len;
    }
    out
}
pub fn lat_lon(s: &[Value]) -> Option<(f64, f64)> {
    let find = |ids: &[u32]| {
        ids.iter()
            .find_map(|id| s.iter().find(|v| v.fxy == *id).and_then(|v| v.value))
    };
    let lat = find(&[5001, 5002])?;
    let lon = find(&[6001, 6002])?;
    (lat.abs() <= 90. && lon.abs() <= 180.).then_some((lat, lon))
}
pub fn main_element(m: &Message) -> Option<(u32, String, String)> {
    m.subsets.iter().flatten().find_map(|v| {
        let x = v.fxy / 1000;
        let e = tables().b.get(&v.fxy)?;
        (v.value.is_some()
            && (10..=30).contains(&x)
            && !e.units.contains("TABLE")
            && !e.units.contains("IA5"))
        .then(|| (v.fxy, e.name.clone(), e.units.clone()))
    })
}
pub fn category_name(c: u8) -> &'static str {
    match c {
        0 => "surface land",
        1 => "surface sea",
        2 => "vertical soundings",
        3 => "satellite soundings",
        4 => "upper-air",
        5 => "satellite observations",
        6 => "radar",
        7 => "synoptic",
        _ => "observations",
    }
}
