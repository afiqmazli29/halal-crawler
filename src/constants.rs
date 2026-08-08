pub const MAX_CONCURRENT: usize = 5;

/// Base64-encoded directory path parameter used in portal requests.
pub const DATA_PARAM: &str = "ZGlyZWN0b3J5L2luZGV4X2RpcmVjdG9yeTs7Ozs=";

/// Malaysian states — used to match state from address strings.
pub const STATES: &[&str] = &[
    "Johor",
    "Kedah",
    "Kelantan",
    "Melaka",
    "Negeri Sembilan",
    "Pahang",
    "Pulau Pinang",
    "Perak",
    "Perlis",
    "Selangor",
    "Terengganu",
    "Sabah",
    "Sarawak",
    "Kuala Lumpur",
    "Labuan",
    "Putrajaya",
];
