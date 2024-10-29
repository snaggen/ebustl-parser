use std::{num::ParseIntError, str::FromStr};

use thiserror::Error;
use winnow::{
    self, binary::{be_u8, le_u16}, bytes::take, combinator::repeat, error::{ErrMode, ErrorKind, FromExternalError}, stream::{Stream, ToUsize}, Parser
};

use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("IoError: {0}")]
    IoError(String),
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
    NomParsingError { message: String },
    #[error("Unknown error")]
    Unknown,
}

impl From<std::io::Error> for ParseError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }
}

impl<I> winnow::error::ParseError<I> for ParseError {
    // on one line, we show the error code and the input that caused it
    fn from_error_kind(_: I, kind: ErrorKind) -> Self {
        Self::NomParsingError {
            message: format!("{:?}", kind),
        }
    }

    // if combining multiple errors, we show them one after the other
    fn append(self, _input: I, kind: ErrorKind) -> Self {
        let message = format!("{:?}", kind);
        Self::NomParsingError { message }
    }
}

impl<I, E> FromExternalError<I, E> for ParseError
where
    E: fmt::Display,
{
    fn from_external_error(_: I, kind: ErrorKind, e: E) -> Self {
        let message = format!("{:?}:\n{}", kind, e);
        Self::NomParsingError { message }
    }
}

impl<E> From<ErrMode<E>> for ParseError
where
    E: fmt::Display,
{
    fn from(err: ErrMode<E>) -> Self {
        match err {
            ErrMode::Incomplete(_) => ParseError::Incomplete,
            ErrMode::Backtrack(e) | ErrMode::Cut(e) => Self::NomParsingError {
                message: format!("{}", e),
            },
        }
    }
}

pub type IResult<I, O> = winnow::IResult<I, O, ParseError>;

fn parse_stl(input: &[u8]) -> IResult<&[u8], Stl> {
    let (input, gsi) = parse_gsi_block(input)?;
    let (input, ttis) = repeat(1.., |i| parse_tti_block(gsi.cct, i)).parse_next(input)?;
    Ok((input, Stl { gsi, ttis }))
}

pub fn parse_stl_from_slice(input: &[u8]) -> Result<Stl, ParseError> {
    let (_, stl) = parse_stl(input)?;
    Ok(stl)
}

pub fn take_str<'a, C: ToUsize, Error: winnow::error::ParseError<&'a [u8]>>(
    count: C,
) -> impl Fn(&'a [u8]) -> winnow::IResult<&'a [u8], &'a str, Error> {
    let c = count.to_usize();
    eprintln!("Take str {c}");
    move |i: &[u8]| match i.offset_at(c) {
        Err(i) => Err(ErrMode::Incomplete(i)),
        Ok(index) => {
            let (first, rest) = i.split_at(index);
            Ok((
                rest,
                str::from_utf8(first).map_err(|_err| {
                    ErrMode::Backtrack(Error::from_error_kind(
                        rest,
                        winnow::error::ErrorKind::Fail,
                    ))
                })?,
            ))
        }
    }
}

fn u8_from_str_with_default_if_blank(input: &str, default: u8) -> Result<u8, ParseIntError> {
    if input.trim().is_empty() {
        Ok(default)
    } else {
        u8::from_str(input)
    }
}

fn parse_gsi_block(input: &[u8]) -> IResult<&[u8], GsiBlock> {
    let (input, (codepage, dfc, dsc, cct)) = (
        take_str(3_u16).try_map(u16::from_str),
        take_str(10 - 3 + 1_u16).try_map(DiskFormatCode::parse),
        be_u8.try_map(DisplayStandardCode::parse),
        take(13 - 12 + 1_u16).try_map(CharacterCodeTable::parse),
    )
        .parse_next(input)?;
    let cpn = CodePageNumber::from_u16(codepage).map_err(ErrMode::Backtrack)?;
    let coding = CodePageDecoder::new(codepage).map_err(ErrMode::Backtrack)?;

    let (input, (lc, opt, oet, tpt, tet, tn, tcd, slr, cd, rd, rn, tnb, tns, tng, mnc, mnr, tcs)) =
        (
            take(15 - 14 + 1_u16).try_map(|data| coding.parse(data)),
            take(47 - 16 + 1_u16).try_map(|data| coding.parse(data)),
            take(79 - 48 + 1_u16).try_map(|data| coding.parse(data)),
            take(111 - 80 + 1_u16).try_map(|data| coding.parse(data)),
            take(143 - 112 + 1_u16).try_map(|data| coding.parse(data)),
            take(175 - 144 + 1_u16).try_map(|data| coding.parse(data)),
            take(207 - 176 + 1_u16).try_map(|data| coding.parse(data)),
            take(223 - 208 + 1_u16).try_map(|data| coding.parse(data)),
            take(229 - 224 + 1_u16).try_map(|data| coding.parse(data)),
            take(235 - 230 + 1_u16).try_map(|data| coding.parse(data)),
            take(237 - 236 + 1_u16).try_map(|data| coding.parse(data)),
            take_str(242 - 238 + 1_u16).try_map(u16::from_str),
            take_str(247 - 243 + 1_u16).try_map(u16::from_str),
            take_str(250 - 248 + 1_u16).try_map(u16::from_str),
            take_str(252 - 251 + 1_u16).try_map(u16::from_str),
            take_str(254 - 253 + 1_u16).try_map(u16::from_str),
            be_u8.try_map(TimeCodeStatus::parse),
        )
            .parse_next(input)?;

    let (input, (tcp, tcf, tnd, dsn, co, pub_, en, ecd, _spare, uda)) = (
        take(263 - 256 + 1_u16).try_map(|data| coding.parse(data)),
        take(271 - 264 + 1_u16).try_map(|data| coding.parse(data)),
        take_str(1_u16).try_map(|data| u8_from_str_with_default_if_blank(data, 1)),
        take_str(1_u16).try_map(|data| u8_from_str_with_default_if_blank(data, 1)),
        take(276 - 274 + 1_u16).try_map(|data| coding.parse(data)),
        take(308 - 277 + 1_u16).try_map(|data| coding.parse(data)),
        take(340 - 309 + 1_u16).try_map(|data| coding.parse(data)),
        take(372 - 341 + 1_u16).try_map(|data| coding.parse(data)),
        take(447 - 373 + 1_u16).try_map(|data| coding.parse(data)),
        take(1023 - 448 + 1_u16).try_map(|data| coding.parse(data)),
    )
        .parse_next(input)?;
    Ok((
        input,
        GsiBlock {
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
        },
    ))
}

fn parse_time(input: &[u8]) -> IResult<&[u8], Time> {
    let (input, (h, m, s, f)) = (be_u8, be_u8, be_u8, be_u8).parse_next(input)?;
    eprintln!("parse time {h} {m} {s} {f}");
    Ok((input, Time::new(h, m, s, f)))
}

fn parse_tti_block(cct: CharacterCodeTable, input: &[u8]) -> IResult<&[u8], TtiBlock> {
    //Needed to handle the many1 operator, that expects an error when done.
    if input.is_empty() {
        return Err(ErrMode::Backtrack(
            winnow::error::ParseError::from_error_kind(input, winnow::error::ErrorKind::Eof),
        ));
    }
    let (input, (sgn, sn, ebn, cs, tci, tco, vp, jc, cf, tf)) = (
        be_u8,
        le_u16,
        be_u8,
        be_u8.try_map(CumulativeStatus::parse),
        parse_time,
        parse_time,
        be_u8,
        be_u8,
        be_u8,
        take(112_u16),
    )
        .parse_next(input)?;
    Ok((
        input,
        TtiBlock {
            sgn,
            sn,
            ebn,
            cs,
            tci,
            tco,
            vp,
            jc,
            cf,
            tf: tf.to_vec(),
            cct,
        },
    ))
}

#[cfg(test)]
mod tests {
    //use winnow::error::Needed;

    use super::*;

    #[test]
    fn test_parse_time() {
        let empty: &[u8] = &[];
        let ok = [0x1, 0x2, 0x3, 0x4];
        //let incomplete = [0x1];

        let (a, time) = parse_time(&ok).unwrap();
        println!("a {a:?}");
        println!("time {time:?}");
        assert_eq!(
            parse_time(&ok),
            Ok((
                empty,
                Time {
                    hours: 1,
                    minutes: 2,
                    seconds: 3,
                    frames: 4,
                }
            ))
        );
        //FIXME: Ensure we handle Partial
        /*
        let a = parse_partial_time(Partial::new(&incomplete));
        println!("a2 {a:?}");
        assert_eq!(
            parse_time(&incomplete),
            Err(ErrMode::Incomplete(Needed::new(1)))
        );
        */
    }
    //Comented out since the test file is propritary
    #[test]
    fn test_parse_file() {
        let stl = parse_stl_from_file("stls/test.stl")
            .map_err(|err| {
                eprintln!("Error: {}", err);
                err.to_string()
            })
            .expect("Parse stl");
        println!("STL:\n{:?}", stl);
        assert_eq!(1_u8, stl.gsi.tnd);
        assert_eq!(1_u8, stl.gsi.dsn);
        assert_eq!(13, stl.ttis.len());
        assert_eq!(
            "    dans la baie de New York.\r\n",
            stl.ttis.get(11).unwrap().get_text()
        );
    }
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
