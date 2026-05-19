macro_rules! parse_params {
    ($parser:expr) => {
        $crate::macros::parse_params!($parser, { Some($parser.parse_param()?) })
    };

    ($parser:expr, $func:block) => {{
        let mut params = $crate::message::param::Params::default();
        $parser.skip_ws();
        while $parser.take_if_eq(b';').is_some() {
            if let Some(param) = $func {
                params.push(param);
            }
            $parser.skip_ws();
        }
        params
    }};
}

macro_rules! collect_elems_separated_by_comma {
    ($parser:expr, $func:block) => {{
        let mut itens = Vec::with_capacity(1);
        loop {
            $parser.skip_ws();

            itens.push($func);

            if $parser.take_if_eq(b',').is_none() {
                break;
            }
        }
        itens
    }};
}

#[macro_export]
macro_rules! headers {
    () => (
        $crate::message::headers::Headers::new()
    );
    ($($x:expr),+ $(,)?) => (
        $crate::message::headers::Headers::from(vec![$($x),+])
    );
}

#[macro_export]
macro_rules! filter_map_header {
    ($hdrs:expr, $header:ty) => {
        $hdrs.iter().filter_map(|hdr| {
            if let $crate::message::headers::Header::$header(v) = hdr {
                Some(v)
            } else {
                None
            }
        })
    };
}

#[macro_export]
macro_rules! find_map_header {
    ($hdrs:expr, $header:ident) => {
        $hdrs.iter().find_map(|hdr| {
            if let $crate::message::headers::Header::$header(v) = hdr {
                Some(v)
            } else {
                None
            }
        })
    };
}

#[macro_export]
macro_rules! find_map_mut_header {
    ($hdrs:expr, $header:ident) => {
        $hdrs.iter_mut().find_map(|hdr| {
            if let $crate::message::headers::Header::$header(v) = hdr {
                Some(v)
            } else {
                None
            }
        })
    };
}

pub(crate) use {collect_elems_separated_by_comma, parse_params};
pub use {filter_map_header, find_map_header, find_map_mut_header, headers};
