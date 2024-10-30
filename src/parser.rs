use std::{num::ParseIntError, str::FromStr};

use thiserror::Error;
use winnow::{
    self,
    binary::{be_u8, le_u16},
    combinator::{repeat, trace},
    error::{ContextError, ErrMode, ErrorKind, FromExternalError, ParserError, StrContext::Label},
    seq,
    stream::ToUsize,
    token::take,
    PResult, Parser,
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

pub fn parse_stl_from_slice(input: &mut &[u8]) -> PResult<Stl> {
    let gsi = parse_gsi_block(input)?;
    let ttis = repeat(1.., parse_tti_block(gsi.cct)).parse_next(input)?;
    Ok(Stl { gsi, ttis })
}

#[inline(always)]
fn take_str<'a, C, Error: ParserError<&'a [u8]>>(count: C) -> impl Parser<&'a [u8], &'a str, Error>
where
    C: ToUsize,
{
    let c = count.to_usize();
    move |i: &mut &'a [u8]| {
        let first = take(c).parse_next(i)?;
        str::from_utf8(first).map_err(|_err| {
            ErrMode::Backtrack(Error::from_error_kind(i, winnow::error::ErrorKind::Fail))
        })
    }
}

fn u8_from_str_with_default_if_blank(input: &str, default: u8) -> Result<u8, ParseIntError> {
    if input.trim().is_empty() {
        Ok(default)
    } else {
        u8::from_str(input)
    }
}

fn parse_gsi_block(input: &mut &[u8]) -> PResult<GsiBlock> {
    let codepage: u16 = trace(
        "codepage",
        take_str(3_u16)
            .try_map(u16::from_str)
            .context(Label("codepage")),
    )
    .parse_next(input)?;
    let cpn = CodePageNumber::from_u16(codepage)
        .map_err(|err| ErrMode::from_external_error(&input, ErrorKind::Fail, err))?;
    let coding = CodePageDecoder::new(codepage)
        .map_err(|err| ErrMode::from_external_error(&input, ErrorKind::Fail, err))?;

    seq!(GsiBlock {
        cpn:
            ().try_map(|_i| Ok::<CodePageNumber, std::convert::Infallible>(cpn))
                .context(Label("cpn")),
        dfc: take_str(10 - 3 + 1_u16)
            .try_map(DiskFormatCode::parse)
            .context(Label("dfc")),
        dsc: be_u8
            .try_map(DisplayStandardCode::parse)
            .context(Label("dsc")),
        cct: take(13 - 12 + 1_u16)
            .try_map(CharacterCodeTable::parse)
            .context(Label("cct")),
        lc: take(15 - 14 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("lc")),
        opt: take(47 - 16 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("opt")),
        oet: take(79 - 48 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("oet")),
        tpt: take(111 - 80 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("tpt")),
        tet: take(143 - 112 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("tet")),
        tn: take(175 - 144 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("tn")),
        tcd: take(207 - 176 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("tcd")),
        slr: take(223 - 208 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("slr")),
        cd: take(229 - 224 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("cd")),
        rd: take(235 - 230 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("rd")),
        rn: take(237 - 236 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("rn")),
        tnb: take_str(242 - 238 + 1_u16)
            .try_map(u16::from_str)
            .context(Label("tnb")),
        tns: take_str(247 - 243 + 1_u16)
            .try_map(u16::from_str)
            .context(Label("tns")),
        tng: take_str(250 - 248 + 1_u16)
            .try_map(u16::from_str)
            .context(Label("tng")),
        mnc: take_str(252 - 251 + 1_u16)
            .try_map(u16::from_str)
            .context(Label("mnc")),
        mnr: take_str(254 - 253 + 1_u16)
            .try_map(u16::from_str)
            .context(Label("mnr")),
        tcs: be_u8.try_map(TimeCodeStatus::parse).context(Label("tcs")),
        tcp: take(263 - 256 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("tcp")),
        tcf: take(271 - 264 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("tcf")),
        tnd: take_str(1_u16)
            .try_map(|data| u8_from_str_with_default_if_blank(data, 1))
            .context(Label("tnd")),
        dsn: take_str(1_u16)
            .try_map(|data| u8_from_str_with_default_if_blank(data, 1))
            .context(Label("dns")),
        co: take(276 - 274 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("co")),
        pub_: take(308 - 277 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("pub_")),
        en: take(340 - 309 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("en")),
        ecd: take(372 - 341 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("ecd")),
        _spare: take(447 - 373 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("_spare")),
        uda: take(1023 - 448 + 1_u16)
            .try_map(|data| coding.parse(data))
            .context(Label("uda")),
    })
    .context(Label("GsiBlock"))
    .parse_next(input)
}

fn parse_time(input: &mut &[u8]) -> PResult<Time> {
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
) -> impl Parser<&'a [u8], TtiBlock, ContextError> {
    move |input: &mut &'a [u8]| {
        if input.is_empty() {
            return Err(ErrMode::Backtrack(
                winnow::error::ParserError::from_error_kind(input, winnow::error::ErrorKind::Eof),
            ));
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
    fn test_parse_file() {
        let stl = parse_stl_from_file("stls/test.stl")
            .map_err(|err| {
                eprintln!("Error: {}", err);
                err.to_string()
            })
            .expect("Parse stl");
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
    }

    //Ignored since the test file is propritary
    #[test]
    #[ignore]
    fn test_parse_proprietary_file() {
        let stl = parse_stl_from_file("stls/proprietary.stl")
            .map_err(|err| {
                eprintln!("Error: {}", err);
                err.to_string()
            })
            .expect("Parse stl");
        println!("STL:\n{:?}", stl);
        //        assert_eq!(1_u8, stl.gsi.tnd);
        //        assert_eq!(1_u8, stl.gsi.dsn);
        //        assert_eq!(28, stl.ttis.len());
        //        assert_eq!(
        //            "سوف تقوم بتدريبات بسيطة جدا\u{64b} اليوم",
        //            stl.ttis.get(11).unwrap().get_text()
        //        );
    }
    /* TODO
    #[test]
    fn test_parse_tti() {
    }
    fn test_parse_gsi() {
    }
    */
}
