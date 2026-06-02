use super::*;
use crate::{Result, uri_test_ok};

uri_test_ok! {
    name: uri_test_1,
    input: "sip:biloxi.com",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .host("biloxi.com".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_2,
    input: "sip:biloxi.com:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .host("biloxi.com:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_3,
    input: "sip:a@b:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "a".to_owned(), pass: None})
        .host("b:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_4,
    input: "sip:bob@biloxi.com:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_5,
    input: "sip:bob@192.0.2.201:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("192.0.2.201:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_6,
    input: "sip:bob@[::1]:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("[::1]:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_7,
    input: "sip:bob:secret@biloxi.com",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: Some("secret".to_owned())})
        .host("biloxi.com".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_8,
    input: "sip:bob:pass@192.0.2.201",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: Some("pass".to_owned())})
        .host("192.0.2.201".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_9,
    input: "sip:bob@biloxi.com;foo=bar",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com".parse().unwrap())
        .param("foo".to_owned(), Some("bar".to_owned()))
        .build()
}

uri_test_ok! {
    name: uri_test_10,
    input: "sip:bob@biloxi.com:5060;foo=bar",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .param("foo".to_owned(), Some("bar".to_owned()))
        .build()
}

uri_test_ok! {
    name: uri_test_11,
    input: "sips:bob@biloxi.com:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sips)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: uri_test_12,
    input: "sips:bob:pass@biloxi.com:5060",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sips)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: Some("pass".to_owned()) })
        .host("biloxi.com:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: test_uri_11,
    input: "sip:bob@biloxi.com:5060;foo",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .param("foo".to_owned(), None)
        .host("biloxi.com:5060".parse().unwrap())
        .build()
}

uri_test_ok! {
    name: test_uri_12,
    input: "sip:bob@biloxi.com:5060;foo;baz=bar",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .param("baz".to_owned(), Some("bar".to_owned()))
        .build()
}

uri_test_ok! {
    name: test_uri_13,
    input: "sip:bob@biloxi.com:5060;baz=bar;foo",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .param("baz".to_owned(), Some("bar".to_owned()))
        .build()
}

uri_test_ok! {
    name: test_uri_14,
    input: "sip:bob@biloxi.com:5060;baz=bar;foo;a=b",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .param("baz".to_owned(), Some("bar".to_owned()))
        .param("foo".to_owned(), None)
        .param("a".to_owned(), Some("b".to_owned()))
        .build()
}

uri_test_ok! {
    name: test_uri_15,
    input: "sip:bob@biloxi.com?foo=bar",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com".parse().unwrap())
        .header("foo".to_owned(), Some("bar".to_owned()))
        .build()
}

uri_test_ok! {
    name: test_uri_16,
    input: "sip:bob@biloxi.com?foo",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com".parse().unwrap())
        .header("foo".to_owned(), None)
        .build()
}

uri_test_ok! {
    name: test_uri_17,
    input: "sip:bob@biloxi.com:5060?foo=bar",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .header("foo".to_owned(), Some("bar".to_owned()))
        .build()
}

uri_test_ok! {
    name: test_uri_18,
    input: "sip:bob@biloxi.com:5060?baz=bar&foo=&a=b",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .header("baz".to_owned(), Some("bar".to_owned()))
        .header("foo".to_owned(), Some("".to_owned()))
        .header("a".to_owned(), Some("b".to_owned()))
        .build()
}

uri_test_ok! {
    name: test_uri_19,
    input: "sip:bob@biloxi.com:5060?foo=bar&baz",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com:5060".parse().unwrap())
        .header("foo".to_owned(), Some("bar".to_owned()))
        .header("baz".to_owned(), None)
        .build()
}

uri_test_ok! {
    name: test_uri_20,
    input: "sip:bob@biloxi.com;foo?foo=bar",
    expected: sip_uri::Uri::builder()
        .scheme(sip_uri::Scheme::Sip)
        .user(sip_uri::UserInfo { user: "bob".to_owned(), pass: None})
        .host("biloxi.com".parse().unwrap())
        .param("foo".to_owned(), None)
        .header("foo".to_owned(), Some("bar".to_owned()))
        .build()
}
