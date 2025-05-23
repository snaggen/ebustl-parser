use std::{num::ParseIntError, str::FromStr};

use codepage_strings::ConvertError;
use thiserror::Error;
use winnow::{
    self, ModalParser, ModalResult, Parser,
    binary::{be_u8, le_u16},
    combinator::{repeat, trace},
    error::{ContextError, ErrMode, FromExternalError, ParserError, StrContext::Label},
    seq,
    stream::ToUsize,
    token::take,
};

use super::*;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error("Error parsing, file may be incomplete or corrupted")]
    Incomplete,
    #[error("Unknown Code Page Number: {0}")]
    CodePageNumber(u16),
    #[error("Error parsing Display Standard Code")]
    DisplayStandardCode,
    #[error("Error parsing Time Code Status")]
    TimeCodeStatus,
    #[error("Error parsing Disk Format Code: {0}")]
    DiskFormatCode(String),
    #[error("Error parsing Character Code Table")]
    CharacterCodeTable,
    #[error("Error parsing Cumulative Status")]
    CumulativeStatus,
    #[error("Parse error: {message}")]
    WinnowParsingError { message: String },
    #[error("Failed to encode string '{value}' using codepage {codepage}: {source}")]
    CodePageEncoding {
        codepage: u16,
        value: String,
        source: ConvertError,
    },
}

impl<E> From<ErrMode<E>> for ParseError
where
    E: fmt::Display,
{
    fn from(err: ErrMode<E>) -> Self {
        match err {
            ErrMode::Incomplete(_) => ParseError::Incomplete,
            ErrMode::Backtrack(e) | ErrMode::Cut(e) => Self::WinnowParsingError {
                message: e.to_string(),
            },
        }
    }
}

/// Parse binary data in the form of bytes array, in to a [Stl] struct
///
/// # Example
///
/// ```rust,no_run
/// use ebustl_parser::parser::parse_stl_from_slice;
/// use std::fs::File;
/// use std::io::Read;
///
/// let mut f = File::open("/path/to/subtitle.stl").expect("Open subtitle file");
/// let mut buffer = vec![];
/// f.read_to_end(&mut buffer).expect("Read to end");
///
/// let stl = parse_stl_from_slice(&mut buffer.as_slice()).expect("Parse stl from slice");
/// println!("{:?}", stl);
/// ```
pub fn parse_stl_from_slice(input: &mut &[u8]) -> ModalResult<Stl> {
    let gsi = parse_gsi_block(input)?;
    let ttis = repeat(1.., parse_tti_block(gsi.cct)).parse_next(input)?;
    Ok(Stl { gsi, ttis })
}

#[inline(always)]
fn take_str<'a, C, Error: ParserError<&'a [u8]>>(
    count: C,
) -> impl ModalParser<&'a [u8], &'a str, Error>
where
    C: ToUsize,
{
    let c = count.to_usize();
    move |i: &mut &'a [u8]| {
        let first = take(c).parse_next(i)?;
        str::from_utf8(first).map_err(|_err| ErrMode::Backtrack(Error::from_input(i)))
    }
}

fn u8_from_str_with_default_if_blank(input: &str, default: u8) -> Result<u8, ParseIntError> {
    if input.trim().is_empty() {
        Ok(default)
    } else {
        u8::from_str(input)
    }
}

fn parse_gsi_block(input: &mut &[u8]) -> ModalResult<GsiBlock> {
    let codepage: u16 = trace(
        "codepage",
        take_str(3_u16)
            .try_map(u16::from_str)
            .context(Label("codepage")),
    )
    .parse_next(input)?;

    let cpn = CodePageNumber::from_u16(codepage)
        .map_err(|err| ErrMode::from_external_error(&input, err))?;

    let coding =
        CodePageCodec::new(codepage).map_err(|err| ErrMode::from_external_error(&input, err))?;

    let dfc = take_str(10 - 3 + 1_u16)
        .try_map(DiskFormatCode::parse)
        .context(Label("dfc"))
        .parse_next(input)?;

    let dsc = be_u8
        .try_map(DisplayStandardCode::parse)
        .context(Label("dsc"))
        .parse_next(input)?;

    let cct = take(13 - 12 + 1_u16)
        .try_map(CharacterCodeTable::parse)
        .context(Label("cct"))
        .parse_next(input)?;

    let lc = take(15 - 14 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("lc"))
        .parse_next(input)?;

    let opt = take(47 - 16 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("opt"))
        .parse_next(input)?;

    let oet = take(79 - 48 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("oet"))
        .parse_next(input)?;

    let tpt = take(111 - 80 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("tpt"))
        .parse_next(input)?;

    let tet = take(143 - 112 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("tet"))
        .parse_next(input)?;

    let tn = take(175 - 144 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("tn"))
        .parse_next(input)?;

    let tcd = take(207 - 176 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("tcd"))
        .parse_next(input)?;

    let slr = take(223 - 208 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("slr"))
        .parse_next(input)?;

    let cd = take(229 - 224 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("cd"))
        .parse_next(input)?;

    let rd = take(235 - 230 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("rd"))
        .parse_next(input)?;

    let rn = take(237 - 236 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("rn"))
        .parse_next(input)?;

    let tnb = take_str(242 - 238 + 1_u16)
        .try_map(u16::from_str)
        .context(Label("tnb"))
        .parse_next(input)?;

    let tns = take_str(247 - 243 + 1_u16)
        .try_map(u16::from_str)
        .context(Label("tns"))
        .parse_next(input)?;

    let tng = take_str(250 - 248 + 1_u16)
        .try_map(u16::from_str)
        .context(Label("tng"))
        .parse_next(input)?;

    let mnc = take_str(252 - 251 + 1_u16)
        .try_map(u16::from_str)
        .context(Label("mnc"))
        .parse_next(input)?;

    let mnr = take_str(254 - 253 + 1_u16)
        .try_map(u16::from_str)
        .context(Label("mnr"))
        .parse_next(input)?;

    let tcs = be_u8
        .try_map(TimeCodeStatus::parse)
        .context(Label("tcs"))
        .parse_next(input)?;

    let tcp = take(263 - 256 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("tcp"))
        .parse_next(input)?;

    let tcf = take(271 - 264 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("tcf"))
        .parse_next(input)?;

    let tnd = take_str(1_u16)
        .try_map(|data| u8_from_str_with_default_if_blank(data, 1))
        .context(Label("tnd"))
        .parse_next(input)?;

    let dsn = take_str(1_u16)
        .try_map(|data| u8_from_str_with_default_if_blank(data, 1))
        .context(Label("dns"))
        .parse_next(input)?;

    let co = take(276 - 274 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("co"))
        .parse_next(input)?;

    let pub_ = take(308 - 277 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("pub_"))
        .parse_next(input)?;

    let en = take(340 - 309 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("en"))
        .parse_next(input)?;

    let ecd = take(372 - 341 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("ecd"))
        .parse_next(input)?;

    let _spare = take(447 - 373 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("_spare"))
        .parse_next(input)?;

    let uda = take(1023 - 448 + 1_u16)
        .try_map(|data| coding.decode(data))
        .context(Label("uda"))
        .parse_next(input)?;

    Ok(GsiBlock {
        cpn,
        dfc,
        dsc,
        cct,
        lc,
        opt,
        oet,
        tpt,
        tet,
        tn,
        tcd,
        slr,
        cd,
        rd,
        rn,
        tnb,
        tns,
        tng,
        mnc,
        mnr,
        tcs,
        tcp,
        tcf,
        tnd,
        dsn,
        co,
        pub_,
        en,
        ecd,
        _spare,
        uda,
    })
}

fn parse_time(input: &mut &[u8]) -> ModalResult<Time> {
    seq!(Time {
        hours: be_u8.context(Label("hours")),
        minutes: be_u8.context(Label("minutes")),
        seconds: be_u8.context(Label("seconds")),
        frames: be_u8.context(Label("frames")),
    })
    .context(Label("Time"))
    .parse_next(input)
}

#[inline(always)]
fn parse_tti_block<'a>(
    cct: CharacterCodeTable,
) -> impl ModalParser<&'a [u8], TtiBlock, ContextError> {
    move |input: &mut &'a [u8]| {
        if input.is_empty() {
            return Err(ErrMode::Backtrack(winnow::error::ParserError::from_input(
                input,
            )));
        }

        seq!(TtiBlock {
            sgn: be_u8.context(Label("sgn")),
            sn: le_u16.context(Label("sn")),
            ebn: be_u8.context(Label("ebn")),
            cs: be_u8.try_map(CumulativeStatus::parse).context(Label("cs")),
            tci: parse_time.context(Label("tci")),
            tco: parse_time.context(Label("tco")),
            vp: be_u8.context(Label("vp")),
            jc: be_u8.context(Label("jc")),
            cf: be_u8.context(Label("cf")),
            tf: take(112_u16)
                .map(|a: &[u8]| a.to_vec())
                .context(Label("tf")),
            cct: ().map(|_i| cct).context(Label("cct")),
        })
        .context(Label("TtiBlock"))
        .parse_next(input)
    }
}

#[cfg(test)]
mod tests {
    use walkdir::WalkDir;

    use super::*;

    #[test]
    fn test_parse_time() {
        let ok = [0x1, 0x2, 0x3, 0x4];

        let time = parse_time(&mut ok.as_slice()).unwrap();
        println!("time {time:?}");
        assert_eq!(
            parse_time(&mut ok.as_slice()),
            Ok(Time {
                hours: 1,
                minutes: 2,
                seconds: 3,
                frames: 4,
            })
        );
    }

    #[test]
    fn parse_basic_file() {
        let mut f = File::open("stls/test.stl").expect("Open stls/test.stl");
        let mut buffer = vec![];
        f.read_to_end(&mut buffer).expect("Read to end");

        let stl = parse_stl_from_slice(&mut buffer.as_slice())
            .map_err(|err| {
                eprintln!("Error: {}", err);
                err.to_string()
            })
            .expect("parse_stl_from_slice");
        let stl2 = parse_stl_from_file("stls/test.stl").expect("parse_stl_from_file");

        println!("STL:\n{:?}", stl);
        assert_eq!(CodePageNumber::CPN_850, stl.gsi.cpn);
        assert_eq!(1_u8, stl.gsi.tnd);
        assert_eq!(1_u8, stl.gsi.dsn);
        assert_eq!("TESTSUB 1.0.1                   ", stl.gsi.en);
        assert_eq!(13, stl.ttis.len());
        assert_eq!(
            "    dans la baie de New York.\r\n",
            stl.ttis.get(11).unwrap().get_text()
        );

        assert_eq!(stl, stl2);
    }

    fn roundtrip_file<P>(filename: P) -> Result<Stl, ParseError>
    where
        P: AsRef<Path>,
        P: fmt::Debug,
    {
        let filepath = filename.as_ref();
        let mut f = File::open(filepath).unwrap_or_else(|_| panic!("Open file {filepath:?}"));
        let mut buffer = vec![];
        f.read_to_end(&mut buffer).expect("Read to end");

        let stl = parse_stl_from_slice(&mut buffer.as_slice())
            .map_err(|err| {
                eprintln!("Error: {}", err);
                err.to_string()
            })
            .expect("Parse stl");

        let mut serialized = stl.gsi.serialize().expect("Serialize GSI");
        stl.ttis
            .iter()
            .for_each(|tti| serialized.append(&mut tti.serialize()));
        assert_eq!(buffer, serialized);
        Ok(stl)
    }
    #[test]
    fn roundtrip_basic_file() {
        roundtrip_file("stls/test.stl").expect("roundtrip stls/test.stl");
    }

    // Test to test basic parsing against a non-public subtitle test file library
    #[test]
    fn test_local_file_library() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(base_folder) = std::env::var("EBUSTL_PARSER_STL_TEST_FILES") else {
            return Ok(());
        };

        println!("Will walk {base_folder} and try to parse all stl files found");
        for entry in WalkDir::new(base_folder).into_iter().filter_map(|e| e.ok()) {
            let Some(filename) = entry.file_name().to_str() else {
                continue;
            };
            if filename.starts_with('.') || !filename.to_lowercase().ends_with(".stl") {
                continue;
            }
            println!("Roundtrip file {:?}", entry.path());
            let stl = roundtrip_file(entry.path())?;
            println!(
                "Roundtripped file {:?} of codepage {:?}",
                entry.path(),
                stl.gsi.get_code_page_number()
            );
            if !stl.ttis.is_empty() {
                let text = stl
                    .ttis
                    .iter()
                    .find(|a| !a.get_text().is_empty())
                    .map(|tti| tti.get_text())
                    .unwrap_or_else(|| {
                        panic!("{:?} doesn't have any non-empty text blocks", entry.path())
                    });
                let first_line = text
                    .lines()
                    .next()
                    .unwrap_or_else(|| panic!("{:?} doesn't have a first text line", entry.path()));
                println!("Test library file {filename}: {}", first_line);
            }
        }
        Ok(())
    }
}
