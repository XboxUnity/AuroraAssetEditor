mod support;

use aurora_asset::{Compression, encode};

const BLOCK_SIDE: u32 = 4;
const BLOCK_BYTES: usize = 16;
const COLOR_HALF: std::ops::Range<usize> = 8..16;

/// Fixtures whose color half must match the DLL on every interior block.
const EXACT_COLOR_FIXTURES: [&str; 4] = ["ph_cover", "ph_background", "ph_icon", "ph_screenshot"];

struct Surfaces {
    ours: Vec<u8>,
    dll: Vec<u8>,
    row_pitch: usize,
    blocks_wide: usize,
    blocks_high: usize,
    interior_wide: usize,
    interior_high: usize,
}

/// Both surfaces in the little-endian D3D block layout, so blocks compare by their field bytes.
fn surfaces(fixture: &support::Fixture) -> Surfaces {
    let texture = encode(&support::input_rgba(&fixture.name), Compression::Dxt).unwrap();
    let mut ours = texture.data;
    let mut dll = support::dxt_video(&fixture.name);
    swap_8in16(&mut ours);
    swap_8in16(&mut dll);
    let blocks_wide = fixture.width.div_ceil(BLOCK_SIDE) as usize;
    let blocks_high = fixture.height.div_ceil(BLOCK_SIDE) as usize;
    Surfaces {
        row_pitch: ours.len() / padded_block_rows(blocks_high),
        ours,
        dll,
        blocks_wide,
        blocks_high,
        interior_wide: (fixture.width / BLOCK_SIDE) as usize,
        interior_high: (fixture.height / BLOCK_SIDE) as usize,
    }
}

/// The surface is padded to a whole number of 32-block rows.
fn padded_block_rows(blocks_high: usize) -> usize {
    const ROW_ALIGNMENT: usize = 32;
    blocks_high.div_ceil(ROW_ALIGNMENT) * ROW_ALIGNMENT
}

fn swap_8in16(data: &mut [u8]) {
    for pair in data.as_chunks_mut::<2>().0.iter_mut() {
        pair.reverse();
    }
}

impl Surfaces {
    fn block<'a>(&self, surface: &'a [u8], bx: usize, by: usize) -> &'a [u8] {
        let start = by * self.row_pitch + bx * BLOCK_BYTES;
        &surface[start..start + BLOCK_BYTES]
    }

    /// Share of the blocks in `0..wide` x `0..high` for which `same` holds.
    fn match_rate(&self, wide: usize, high: usize, same: impl Fn(&[u8], &[u8]) -> bool) -> f64 {
        let mut matching = 0usize;
        for by in 0..high {
            for bx in 0..wide {
                if same(
                    self.block(&self.ours, bx, by),
                    self.block(&self.dll, bx, by),
                ) {
                    matching += 1;
                }
            }
        }
        matching as f64 / (wide * high) as f64
    }

    fn color_match_rate(&self) -> f64 {
        self.match_rate(self.interior_wide, self.interior_high, |ours, dll| {
            ours[COLOR_HALF] == dll[COLOR_HALF]
        })
    }

    fn whole_block_match_rate(&self) -> f64 {
        self.match_rate(self.blocks_wide, self.blocks_high, |ours, dll| ours == dll)
    }
}

fn fixtures_with_interior_blocks() -> Vec<support::Fixture> {
    support::manifest()
        .fixture
        .into_iter()
        .filter(|fixture| fixture.width >= BLOCK_SIDE && fixture.height >= BLOCK_SIDE)
        .collect()
}

#[test]
fn dxt_header_and_size_match_dll() {
    for fixture in support::manifest().fixture {
        let texture = encode(&support::input_rgba(&fixture.name), Compression::Dxt).unwrap();
        assert_eq!(
            texture.header.to_bytes(),
            support::header_bytes(&fixture.dxt_header),
            "{} header",
            fixture.name
        );
        assert_eq!(
            texture.data.len(),
            support::dxt_video(&fixture.name).len(),
            "{} size",
            fixture.name
        );
    }
}

#[test]
fn color_blocks_match_dll_on_placeholders() {
    for name in EXACT_COLOR_FIXTURES {
        let fixture = fixtures_with_interior_blocks()
            .into_iter()
            .find(|fixture| fixture.name == name)
            .unwrap();
        assert_eq!(surfaces(&fixture).color_match_rate(), 1.0, "{name}");
    }
}

#[test]
fn dxt_encode_matches_dll_on_all_fixtures() {
    const LISTED_MISMATCHES: usize = 5;
    let mut mismatches = Vec::new();
    for fixture in support::manifest().fixture {
        let surfaces = surfaces(&fixture);
        let mut listed = 0;
        for by in 0..surfaces.blocks_high {
            for bx in 0..surfaces.blocks_wide {
                let ours = surfaces.block(&surfaces.ours, bx, by);
                let dll = surfaces.block(&surfaces.dll, bx, by);
                if ours != dll && listed < LISTED_MISMATCHES {
                    listed += 1;
                    mismatches.push(format!(
                        "{} block {bx}/{by}: ours {} dll {}",
                        fixture.name,
                        hex(ours),
                        hex(dll)
                    ));
                }
            }
        }
        if surfaces.ours != surfaces.dll && listed == 0 {
            mismatches.push(format!("{}: padding differs", fixture.name));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Run with `--nocapture` to see how close each fixture is to the DLL.
#[test]
fn report_match_rates_against_dll() {
    println!(
        "{:<14} {:>12} {:>12}",
        "fixture", "color half", "whole block"
    );
    for fixture in fixtures_with_interior_blocks() {
        let surfaces = surfaces(&fixture);
        println!(
            "{:<14} {:>12.4} {:>12.4}",
            fixture.name,
            surfaces.color_match_rate(),
            surfaces.whole_block_match_rate()
        );
    }
}
