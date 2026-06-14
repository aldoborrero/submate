//! `LanguageCode` enum and ISO-639 conversions.
//!
//! Direct port of `submate/language.py`. The variant set, the
//! `(iso_639_1, iso_639_2_t, iso_639_2_b, name_en, name_native)` table, and the
//! lookup/conversion semantics match the Python implementation byte-for-byte,
//! including ISO 639-2/B vs /T divergences (e.g. Tibetan `bod`/`tib`, German
//! `deu`/`ger`) and non-Latin native names.
//!
//! The table is hand-rolled rather than sourced from a third-party language
//! crate: those crates carry their own (differing) data, and downstream
//! subtitle/path/config layers require exact parity with the Python tables.

/// Comprehensive language code enum with ISO 639-1, ISO 639-2/T and ISO 639-2/B
/// support, plus English and native names.
///
/// [`LanguageCode::None`] represents an absent/unknown language; it maps to all
/// `None` codes and names, mirroring the Python `NONE` member.
// Variant identifiers deliberately match the Python `LanguageCode` member
// names 1:1 (e.g. `HAITIAN_CREOLE`), which the parity test relies on for its
// name-based fixture mapping. This trades Rust naming convention for exact,
// auditable correspondence with `submate/language.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub enum LanguageCode {
    AFAR,
    AFRIKAANS,
    AMHARIC,
    ARABIC,
    ASSAMESE,
    AZERBAIJANI,
    BASHKIR,
    BELARUSIAN,
    BULGARIAN,
    BENGALI,
    TIBETAN,
    BRETON,
    BOSNIAN,
    CATALAN,
    CZECH,
    WELSH,
    DANISH,
    GERMAN,
    GREEK,
    ENGLISH,
    SPANISH,
    ESTONIAN,
    BASQUE,
    PERSIAN,
    FINNISH,
    FAROESE,
    FRENCH,
    GALICIAN,
    GUJARATI,
    HAUSA,
    HAWAIIAN,
    HEBREW,
    HINDI,
    CROATIAN,
    HAITIAN_CREOLE,
    HUNGARIAN,
    ARMENIAN,
    INDONESIAN,
    ICELANDIC,
    ITALIAN,
    JAPANESE,
    JAVANESE,
    GEORGIAN,
    KAZAKH,
    KHMER,
    KANNADA,
    KOREAN,
    LATIN,
    LUXEMBOURGISH,
    LINGALA,
    LAO,
    LITHUANIAN,
    LATVIAN,
    MALAGASY,
    MAORI,
    MACEDONIAN,
    MALAYALAM,
    MONGOLIAN,
    MARATHI,
    MALAY,
    MALTESE,
    BURMESE,
    NEPALI,
    DUTCH,
    NORWEGIAN_NYNORSK,
    NORWEGIAN,
    OCCITAN,
    PUNJABI,
    POLISH,
    PASHTO,
    PORTUGUESE,
    ROMANIAN,
    RUSSIAN,
    SANSKRIT,
    SINDHI,
    SINHALA,
    SLOVAK,
    SLOVENE,
    SHONA,
    SOMALI,
    ALBANIAN,
    SERBIAN,
    SUNDANESE,
    SWEDISH,
    SWAHILI,
    TAMIL,
    TELUGU,
    TAJIK,
    THAI,
    TURKMEN,
    TAGALOG,
    TURKISH,
    TATAR,
    UKRAINIAN,
    URDU,
    UZBEK,
    VIETNAMESE,
    YIDDISH,
    YORUBA,
    CHINESE,
    CANTONESE,
    /// No / unknown language. All codes and names are `None`.
    None,
}

/// One row of the language table.
struct LangEntry {
    variant: LanguageCode,
    iso_639_1: Option<&'static str>,
    iso_639_2_t: Option<&'static str>,
    iso_639_2_b: Option<&'static str>,
    name_en: Option<&'static str>,
    name_native: Option<&'static str>,
}

/// The full language table, in Python definition order.
///
/// Lookups iterate this slice front-to-back and return the first match, exactly
/// matching the Python `for lang in LanguageCode` resolution order. The
/// `NONE` member is intentionally excluded here; it is the fallback returned
/// when nothing matches.
#[rustfmt::skip]
const TABLE: &[LangEntry] = &[
    LangEntry { variant: LanguageCode::AFAR, iso_639_1: Some("aa"), iso_639_2_t: Some("aar"), iso_639_2_b: Some("aar"), name_en: Some("Afar"), name_native: Some("Afar") },
    LangEntry { variant: LanguageCode::AFRIKAANS, iso_639_1: Some("af"), iso_639_2_t: Some("afr"), iso_639_2_b: Some("afr"), name_en: Some("Afrikaans"), name_native: Some("Afrikaans") },
    LangEntry { variant: LanguageCode::AMHARIC, iso_639_1: Some("am"), iso_639_2_t: Some("amh"), iso_639_2_b: Some("amh"), name_en: Some("Amharic"), name_native: Some("አማርኛ") },
    LangEntry { variant: LanguageCode::ARABIC, iso_639_1: Some("ar"), iso_639_2_t: Some("ara"), iso_639_2_b: Some("ara"), name_en: Some("Arabic"), name_native: Some("العربية") },
    LangEntry { variant: LanguageCode::ASSAMESE, iso_639_1: Some("as"), iso_639_2_t: Some("asm"), iso_639_2_b: Some("asm"), name_en: Some("Assamese"), name_native: Some("অসমীয়া") },
    LangEntry { variant: LanguageCode::AZERBAIJANI, iso_639_1: Some("az"), iso_639_2_t: Some("aze"), iso_639_2_b: Some("aze"), name_en: Some("Azerbaijani"), name_native: Some("Azərbaycanca") },
    LangEntry { variant: LanguageCode::BASHKIR, iso_639_1: Some("ba"), iso_639_2_t: Some("bak"), iso_639_2_b: Some("bak"), name_en: Some("Bashkir"), name_native: Some("Башҡортса") },
    LangEntry { variant: LanguageCode::BELARUSIAN, iso_639_1: Some("be"), iso_639_2_t: Some("bel"), iso_639_2_b: Some("bel"), name_en: Some("Belarusian"), name_native: Some("Беларуская") },
    LangEntry { variant: LanguageCode::BULGARIAN, iso_639_1: Some("bg"), iso_639_2_t: Some("bul"), iso_639_2_b: Some("bul"), name_en: Some("Bulgarian"), name_native: Some("Български") },
    LangEntry { variant: LanguageCode::BENGALI, iso_639_1: Some("bn"), iso_639_2_t: Some("ben"), iso_639_2_b: Some("ben"), name_en: Some("Bengali"), name_native: Some("বাংলা") },
    LangEntry { variant: LanguageCode::TIBETAN, iso_639_1: Some("bo"), iso_639_2_t: Some("bod"), iso_639_2_b: Some("tib"), name_en: Some("Tibetan"), name_native: Some("བོད་ཡིག") },
    LangEntry { variant: LanguageCode::BRETON, iso_639_1: Some("br"), iso_639_2_t: Some("bre"), iso_639_2_b: Some("bre"), name_en: Some("Breton"), name_native: Some("Brezhoneg") },
    LangEntry { variant: LanguageCode::BOSNIAN, iso_639_1: Some("bs"), iso_639_2_t: Some("bos"), iso_639_2_b: Some("bos"), name_en: Some("Bosnian"), name_native: Some("Bosanski") },
    LangEntry { variant: LanguageCode::CATALAN, iso_639_1: Some("ca"), iso_639_2_t: Some("cat"), iso_639_2_b: Some("cat"), name_en: Some("Catalan"), name_native: Some("Català") },
    LangEntry { variant: LanguageCode::CZECH, iso_639_1: Some("cs"), iso_639_2_t: Some("ces"), iso_639_2_b: Some("cze"), name_en: Some("Czech"), name_native: Some("Čeština") },
    LangEntry { variant: LanguageCode::WELSH, iso_639_1: Some("cy"), iso_639_2_t: Some("cym"), iso_639_2_b: Some("wel"), name_en: Some("Welsh"), name_native: Some("Cymraeg") },
    LangEntry { variant: LanguageCode::DANISH, iso_639_1: Some("da"), iso_639_2_t: Some("dan"), iso_639_2_b: Some("dan"), name_en: Some("Danish"), name_native: Some("Dansk") },
    LangEntry { variant: LanguageCode::GERMAN, iso_639_1: Some("de"), iso_639_2_t: Some("deu"), iso_639_2_b: Some("ger"), name_en: Some("German"), name_native: Some("Deutsch") },
    LangEntry { variant: LanguageCode::GREEK, iso_639_1: Some("el"), iso_639_2_t: Some("ell"), iso_639_2_b: Some("gre"), name_en: Some("Greek"), name_native: Some("Ελληνικά") },
    LangEntry { variant: LanguageCode::ENGLISH, iso_639_1: Some("en"), iso_639_2_t: Some("eng"), iso_639_2_b: Some("eng"), name_en: Some("English"), name_native: Some("English") },
    LangEntry { variant: LanguageCode::SPANISH, iso_639_1: Some("es"), iso_639_2_t: Some("spa"), iso_639_2_b: Some("spa"), name_en: Some("Spanish"), name_native: Some("Español") },
    LangEntry { variant: LanguageCode::ESTONIAN, iso_639_1: Some("et"), iso_639_2_t: Some("est"), iso_639_2_b: Some("est"), name_en: Some("Estonian"), name_native: Some("Eesti") },
    LangEntry { variant: LanguageCode::BASQUE, iso_639_1: Some("eu"), iso_639_2_t: Some("eus"), iso_639_2_b: Some("baq"), name_en: Some("Basque"), name_native: Some("Euskara") },
    LangEntry { variant: LanguageCode::PERSIAN, iso_639_1: Some("fa"), iso_639_2_t: Some("fas"), iso_639_2_b: Some("per"), name_en: Some("Persian"), name_native: Some("فارسی") },
    LangEntry { variant: LanguageCode::FINNISH, iso_639_1: Some("fi"), iso_639_2_t: Some("fin"), iso_639_2_b: Some("fin"), name_en: Some("Finnish"), name_native: Some("Suomi") },
    LangEntry { variant: LanguageCode::FAROESE, iso_639_1: Some("fo"), iso_639_2_t: Some("fao"), iso_639_2_b: Some("fao"), name_en: Some("Faroese"), name_native: Some("Føroyskt") },
    LangEntry { variant: LanguageCode::FRENCH, iso_639_1: Some("fr"), iso_639_2_t: Some("fra"), iso_639_2_b: Some("fre"), name_en: Some("French"), name_native: Some("Français") },
    LangEntry { variant: LanguageCode::GALICIAN, iso_639_1: Some("gl"), iso_639_2_t: Some("glg"), iso_639_2_b: Some("glg"), name_en: Some("Galician"), name_native: Some("Galego") },
    LangEntry { variant: LanguageCode::GUJARATI, iso_639_1: Some("gu"), iso_639_2_t: Some("guj"), iso_639_2_b: Some("guj"), name_en: Some("Gujarati"), name_native: Some("ગુજરાતી") },
    LangEntry { variant: LanguageCode::HAUSA, iso_639_1: Some("ha"), iso_639_2_t: Some("hau"), iso_639_2_b: Some("hau"), name_en: Some("Hausa"), name_native: Some("Hausa") },
    LangEntry { variant: LanguageCode::HAWAIIAN, iso_639_1: Some("haw"), iso_639_2_t: Some("haw"), iso_639_2_b: Some("haw"), name_en: Some("Hawaiian"), name_native: Some("ʻŌlelo Hawaiʻi") },
    LangEntry { variant: LanguageCode::HEBREW, iso_639_1: Some("he"), iso_639_2_t: Some("heb"), iso_639_2_b: Some("heb"), name_en: Some("Hebrew"), name_native: Some("עברית") },
    LangEntry { variant: LanguageCode::HINDI, iso_639_1: Some("hi"), iso_639_2_t: Some("hin"), iso_639_2_b: Some("hin"), name_en: Some("Hindi"), name_native: Some("हिन्दी") },
    LangEntry { variant: LanguageCode::CROATIAN, iso_639_1: Some("hr"), iso_639_2_t: Some("hrv"), iso_639_2_b: Some("hrv"), name_en: Some("Croatian"), name_native: Some("Hrvatski") },
    LangEntry { variant: LanguageCode::HAITIAN_CREOLE, iso_639_1: Some("ht"), iso_639_2_t: Some("hat"), iso_639_2_b: Some("hat"), name_en: Some("Haitian Creole"), name_native: Some("Kreyòl Ayisyen") },
    LangEntry { variant: LanguageCode::HUNGARIAN, iso_639_1: Some("hu"), iso_639_2_t: Some("hun"), iso_639_2_b: Some("hun"), name_en: Some("Hungarian"), name_native: Some("Magyar") },
    LangEntry { variant: LanguageCode::ARMENIAN, iso_639_1: Some("hy"), iso_639_2_t: Some("hye"), iso_639_2_b: Some("arm"), name_en: Some("Armenian"), name_native: Some("Հայերեն") },
    LangEntry { variant: LanguageCode::INDONESIAN, iso_639_1: Some("id"), iso_639_2_t: Some("ind"), iso_639_2_b: Some("ind"), name_en: Some("Indonesian"), name_native: Some("Bahasa Indonesia") },
    LangEntry { variant: LanguageCode::ICELANDIC, iso_639_1: Some("is"), iso_639_2_t: Some("isl"), iso_639_2_b: Some("ice"), name_en: Some("Icelandic"), name_native: Some("Íslenska") },
    LangEntry { variant: LanguageCode::ITALIAN, iso_639_1: Some("it"), iso_639_2_t: Some("ita"), iso_639_2_b: Some("ita"), name_en: Some("Italian"), name_native: Some("Italiano") },
    LangEntry { variant: LanguageCode::JAPANESE, iso_639_1: Some("ja"), iso_639_2_t: Some("jpn"), iso_639_2_b: Some("jpn"), name_en: Some("Japanese"), name_native: Some("日本語") },
    LangEntry { variant: LanguageCode::JAVANESE, iso_639_1: Some("jw"), iso_639_2_t: Some("jav"), iso_639_2_b: Some("jav"), name_en: Some("Javanese"), name_native: Some("ꦧꦱꦗꦮ") },
    LangEntry { variant: LanguageCode::GEORGIAN, iso_639_1: Some("ka"), iso_639_2_t: Some("kat"), iso_639_2_b: Some("geo"), name_en: Some("Georgian"), name_native: Some("ქართული") },
    LangEntry { variant: LanguageCode::KAZAKH, iso_639_1: Some("kk"), iso_639_2_t: Some("kaz"), iso_639_2_b: Some("kaz"), name_en: Some("Kazakh"), name_native: Some("Қазақша") },
    LangEntry { variant: LanguageCode::KHMER, iso_639_1: Some("km"), iso_639_2_t: Some("khm"), iso_639_2_b: Some("khm"), name_en: Some("Khmer"), name_native: Some("ភាសាខ្មែរ") },
    LangEntry { variant: LanguageCode::KANNADA, iso_639_1: Some("kn"), iso_639_2_t: Some("kan"), iso_639_2_b: Some("kan"), name_en: Some("Kannada"), name_native: Some("ಕನ್ನಡ") },
    LangEntry { variant: LanguageCode::KOREAN, iso_639_1: Some("ko"), iso_639_2_t: Some("kor"), iso_639_2_b: Some("kor"), name_en: Some("Korean"), name_native: Some("한국어") },
    LangEntry { variant: LanguageCode::LATIN, iso_639_1: Some("la"), iso_639_2_t: Some("lat"), iso_639_2_b: Some("lat"), name_en: Some("Latin"), name_native: Some("Latina") },
    LangEntry { variant: LanguageCode::LUXEMBOURGISH, iso_639_1: Some("lb"), iso_639_2_t: Some("ltz"), iso_639_2_b: Some("ltz"), name_en: Some("Luxembourgish"), name_native: Some("Lëtzebuergesch") },
    LangEntry { variant: LanguageCode::LINGALA, iso_639_1: Some("ln"), iso_639_2_t: Some("lin"), iso_639_2_b: Some("lin"), name_en: Some("Lingala"), name_native: Some("Lingála") },
    LangEntry { variant: LanguageCode::LAO, iso_639_1: Some("lo"), iso_639_2_t: Some("lao"), iso_639_2_b: Some("lao"), name_en: Some("Lao"), name_native: Some("ພາສາລາວ") },
    LangEntry { variant: LanguageCode::LITHUANIAN, iso_639_1: Some("lt"), iso_639_2_t: Some("lit"), iso_639_2_b: Some("lit"), name_en: Some("Lithuanian"), name_native: Some("Lietuvių") },
    LangEntry { variant: LanguageCode::LATVIAN, iso_639_1: Some("lv"), iso_639_2_t: Some("lav"), iso_639_2_b: Some("lav"), name_en: Some("Latvian"), name_native: Some("Latviešu") },
    LangEntry { variant: LanguageCode::MALAGASY, iso_639_1: Some("mg"), iso_639_2_t: Some("mlg"), iso_639_2_b: Some("mlg"), name_en: Some("Malagasy"), name_native: Some("Malagasy") },
    LangEntry { variant: LanguageCode::MAORI, iso_639_1: Some("mi"), iso_639_2_t: Some("mri"), iso_639_2_b: Some("mao"), name_en: Some("Maori"), name_native: Some("Te Reo Māori") },
    LangEntry { variant: LanguageCode::MACEDONIAN, iso_639_1: Some("mk"), iso_639_2_t: Some("mkd"), iso_639_2_b: Some("mac"), name_en: Some("Macedonian"), name_native: Some("Македонски") },
    LangEntry { variant: LanguageCode::MALAYALAM, iso_639_1: Some("ml"), iso_639_2_t: Some("mal"), iso_639_2_b: Some("mal"), name_en: Some("Malayalam"), name_native: Some("മലയാളം") },
    LangEntry { variant: LanguageCode::MONGOLIAN, iso_639_1: Some("mn"), iso_639_2_t: Some("mon"), iso_639_2_b: Some("mon"), name_en: Some("Mongolian"), name_native: Some("Монгол") },
    LangEntry { variant: LanguageCode::MARATHI, iso_639_1: Some("mr"), iso_639_2_t: Some("mar"), iso_639_2_b: Some("mar"), name_en: Some("Marathi"), name_native: Some("मराठी") },
    LangEntry { variant: LanguageCode::MALAY, iso_639_1: Some("ms"), iso_639_2_t: Some("msa"), iso_639_2_b: Some("may"), name_en: Some("Malay"), name_native: Some("Bahasa Melayu") },
    LangEntry { variant: LanguageCode::MALTESE, iso_639_1: Some("mt"), iso_639_2_t: Some("mlt"), iso_639_2_b: Some("mlt"), name_en: Some("Maltese"), name_native: Some("Malti") },
    LangEntry { variant: LanguageCode::BURMESE, iso_639_1: Some("my"), iso_639_2_t: Some("mya"), iso_639_2_b: Some("bur"), name_en: Some("Burmese"), name_native: Some("မြန်မာစာ") },
    LangEntry { variant: LanguageCode::NEPALI, iso_639_1: Some("ne"), iso_639_2_t: Some("nep"), iso_639_2_b: Some("nep"), name_en: Some("Nepali"), name_native: Some("नेपाली") },
    LangEntry { variant: LanguageCode::DUTCH, iso_639_1: Some("nl"), iso_639_2_t: Some("nld"), iso_639_2_b: Some("dut"), name_en: Some("Dutch"), name_native: Some("Nederlands") },
    LangEntry { variant: LanguageCode::NORWEGIAN_NYNORSK, iso_639_1: Some("nn"), iso_639_2_t: Some("nno"), iso_639_2_b: Some("nno"), name_en: Some("Norwegian Nynorsk"), name_native: Some("Nynorsk") },
    LangEntry { variant: LanguageCode::NORWEGIAN, iso_639_1: Some("no"), iso_639_2_t: Some("nor"), iso_639_2_b: Some("nor"), name_en: Some("Norwegian"), name_native: Some("Norsk") },
    LangEntry { variant: LanguageCode::OCCITAN, iso_639_1: Some("oc"), iso_639_2_t: Some("oci"), iso_639_2_b: Some("oci"), name_en: Some("Occitan"), name_native: Some("Occitan") },
    LangEntry { variant: LanguageCode::PUNJABI, iso_639_1: Some("pa"), iso_639_2_t: Some("pan"), iso_639_2_b: Some("pan"), name_en: Some("Punjabi"), name_native: Some("ਪੰਜਾਬੀ") },
    LangEntry { variant: LanguageCode::POLISH, iso_639_1: Some("pl"), iso_639_2_t: Some("pol"), iso_639_2_b: Some("pol"), name_en: Some("Polish"), name_native: Some("Polski") },
    LangEntry { variant: LanguageCode::PASHTO, iso_639_1: Some("ps"), iso_639_2_t: Some("pus"), iso_639_2_b: Some("pus"), name_en: Some("Pashto"), name_native: Some("پښتو") },
    LangEntry { variant: LanguageCode::PORTUGUESE, iso_639_1: Some("pt"), iso_639_2_t: Some("por"), iso_639_2_b: Some("por"), name_en: Some("Portuguese"), name_native: Some("Português") },
    LangEntry { variant: LanguageCode::ROMANIAN, iso_639_1: Some("ro"), iso_639_2_t: Some("ron"), iso_639_2_b: Some("rum"), name_en: Some("Romanian"), name_native: Some("Română") },
    LangEntry { variant: LanguageCode::RUSSIAN, iso_639_1: Some("ru"), iso_639_2_t: Some("rus"), iso_639_2_b: Some("rus"), name_en: Some("Russian"), name_native: Some("Русский") },
    LangEntry { variant: LanguageCode::SANSKRIT, iso_639_1: Some("sa"), iso_639_2_t: Some("san"), iso_639_2_b: Some("san"), name_en: Some("Sanskrit"), name_native: Some("संस्कृतम्") },
    LangEntry { variant: LanguageCode::SINDHI, iso_639_1: Some("sd"), iso_639_2_t: Some("snd"), iso_639_2_b: Some("snd"), name_en: Some("Sindhi"), name_native: Some("سنڌي") },
    LangEntry { variant: LanguageCode::SINHALA, iso_639_1: Some("si"), iso_639_2_t: Some("sin"), iso_639_2_b: Some("sin"), name_en: Some("Sinhala"), name_native: Some("සිංහල") },
    LangEntry { variant: LanguageCode::SLOVAK, iso_639_1: Some("sk"), iso_639_2_t: Some("slk"), iso_639_2_b: Some("slo"), name_en: Some("Slovak"), name_native: Some("Slovenčina") },
    LangEntry { variant: LanguageCode::SLOVENE, iso_639_1: Some("sl"), iso_639_2_t: Some("slv"), iso_639_2_b: Some("slv"), name_en: Some("Slovene"), name_native: Some("Slovenščina") },
    LangEntry { variant: LanguageCode::SHONA, iso_639_1: Some("sn"), iso_639_2_t: Some("sna"), iso_639_2_b: Some("sna"), name_en: Some("Shona"), name_native: Some("ChiShona") },
    LangEntry { variant: LanguageCode::SOMALI, iso_639_1: Some("so"), iso_639_2_t: Some("som"), iso_639_2_b: Some("som"), name_en: Some("Somali"), name_native: Some("Soomaaliga") },
    LangEntry { variant: LanguageCode::ALBANIAN, iso_639_1: Some("sq"), iso_639_2_t: Some("sqi"), iso_639_2_b: Some("alb"), name_en: Some("Albanian"), name_native: Some("Shqip") },
    LangEntry { variant: LanguageCode::SERBIAN, iso_639_1: Some("sr"), iso_639_2_t: Some("srp"), iso_639_2_b: Some("srp"), name_en: Some("Serbian"), name_native: Some("Српски") },
    LangEntry { variant: LanguageCode::SUNDANESE, iso_639_1: Some("su"), iso_639_2_t: Some("sun"), iso_639_2_b: Some("sun"), name_en: Some("Sundanese"), name_native: Some("Basa Sunda") },
    LangEntry { variant: LanguageCode::SWEDISH, iso_639_1: Some("sv"), iso_639_2_t: Some("swe"), iso_639_2_b: Some("swe"), name_en: Some("Swedish"), name_native: Some("Svenska") },
    LangEntry { variant: LanguageCode::SWAHILI, iso_639_1: Some("sw"), iso_639_2_t: Some("swa"), iso_639_2_b: Some("swa"), name_en: Some("Swahili"), name_native: Some("Kiswahili") },
    LangEntry { variant: LanguageCode::TAMIL, iso_639_1: Some("ta"), iso_639_2_t: Some("tam"), iso_639_2_b: Some("tam"), name_en: Some("Tamil"), name_native: Some("தமிழ்") },
    LangEntry { variant: LanguageCode::TELUGU, iso_639_1: Some("te"), iso_639_2_t: Some("tel"), iso_639_2_b: Some("tel"), name_en: Some("Telugu"), name_native: Some("తెలుగు") },
    LangEntry { variant: LanguageCode::TAJIK, iso_639_1: Some("tg"), iso_639_2_t: Some("tgk"), iso_639_2_b: Some("tgk"), name_en: Some("Tajik"), name_native: Some("Тоҷикӣ") },
    LangEntry { variant: LanguageCode::THAI, iso_639_1: Some("th"), iso_639_2_t: Some("tha"), iso_639_2_b: Some("tha"), name_en: Some("Thai"), name_native: Some("ไทย") },
    LangEntry { variant: LanguageCode::TURKMEN, iso_639_1: Some("tk"), iso_639_2_t: Some("tuk"), iso_639_2_b: Some("tuk"), name_en: Some("Turkmen"), name_native: Some("Türkmençe") },
    LangEntry { variant: LanguageCode::TAGALOG, iso_639_1: Some("tl"), iso_639_2_t: Some("tgl"), iso_639_2_b: Some("tgl"), name_en: Some("Tagalog"), name_native: Some("Tagalog") },
    LangEntry { variant: LanguageCode::TURKISH, iso_639_1: Some("tr"), iso_639_2_t: Some("tur"), iso_639_2_b: Some("tur"), name_en: Some("Turkish"), name_native: Some("Türkçe") },
    LangEntry { variant: LanguageCode::TATAR, iso_639_1: Some("tt"), iso_639_2_t: Some("tat"), iso_639_2_b: Some("tat"), name_en: Some("Tatar"), name_native: Some("Татарча") },
    LangEntry { variant: LanguageCode::UKRAINIAN, iso_639_1: Some("uk"), iso_639_2_t: Some("ukr"), iso_639_2_b: Some("ukr"), name_en: Some("Ukrainian"), name_native: Some("Українська") },
    LangEntry { variant: LanguageCode::URDU, iso_639_1: Some("ur"), iso_639_2_t: Some("urd"), iso_639_2_b: Some("urd"), name_en: Some("Urdu"), name_native: Some("اردو") },
    LangEntry { variant: LanguageCode::UZBEK, iso_639_1: Some("uz"), iso_639_2_t: Some("uzb"), iso_639_2_b: Some("uzb"), name_en: Some("Uzbek"), name_native: Some("Oʻzbek") },
    LangEntry { variant: LanguageCode::VIETNAMESE, iso_639_1: Some("vi"), iso_639_2_t: Some("vie"), iso_639_2_b: Some("vie"), name_en: Some("Vietnamese"), name_native: Some("Tiếng Việt") },
    LangEntry { variant: LanguageCode::YIDDISH, iso_639_1: Some("yi"), iso_639_2_t: Some("yid"), iso_639_2_b: Some("yid"), name_en: Some("Yiddish"), name_native: Some("ייִדיש") },
    LangEntry { variant: LanguageCode::YORUBA, iso_639_1: Some("yo"), iso_639_2_t: Some("yor"), iso_639_2_b: Some("yor"), name_en: Some("Yoruba"), name_native: Some("Yorùbá") },
    LangEntry { variant: LanguageCode::CHINESE, iso_639_1: Some("zh"), iso_639_2_t: Some("zho"), iso_639_2_b: Some("chi"), name_en: Some("Chinese"), name_native: Some("中文") },
    LangEntry { variant: LanguageCode::CANTONESE, iso_639_1: Some("yue"), iso_639_2_t: Some("yue"), iso_639_2_b: Some("yue"), name_en: Some("Cantonese"), name_native: Some("粵語") },
];

impl LanguageCode {
    /// All variants in definition order, excluding [`LanguageCode::None`].
    /// Useful for parity tests that enumerate the full table.
    pub fn all() -> impl Iterator<Item = Self> {
        TABLE.iter().map(|e| e.variant)
    }

    fn entry(self) -> Option<&'static LangEntry> {
        TABLE.iter().find(|e| e.variant == self)
    }

    /// ISO 639-1 code (e.g. `"en"`), or `None`.
    pub fn to_iso_639_1(self) -> Option<&'static str> {
        self.entry().and_then(|e| e.iso_639_1)
    }

    /// ISO 639-2/T code (e.g. `"eng"`), or `None`.
    pub fn to_iso_639_2_t(self) -> Option<&'static str> {
        self.entry().and_then(|e| e.iso_639_2_t)
    }

    /// ISO 639-2/B code (e.g. `"ger"` for German), or `None`.
    pub fn to_iso_639_2_b(self) -> Option<&'static str> {
        self.entry().and_then(|e| e.iso_639_2_b)
    }

    /// Language name. English name when `in_english`, otherwise the native name.
    pub fn to_name(self, in_english: bool) -> Option<&'static str> {
        let e = self.entry()?;
        if in_english {
            e.name_en
        } else {
            e.name_native
        }
    }

    /// English name (`name_en`), or `None`.
    pub fn name_en(self) -> Option<&'static str> {
        self.to_name(true)
    }

    /// Native name (`name_native`), or `None`.
    pub fn name_native(self) -> Option<&'static str> {
        self.to_name(false)
    }

    /// Look up by ISO 639-1 code (e.g. `"en"`). Case-insensitive and trimmed.
    /// Empty/`None` input yields [`LanguageCode::None`].
    pub fn from_iso_639_1(code: Option<&str>) -> Self {
        let Some(code) = normalize(code) else {
            return Self::None;
        };
        TABLE
            .iter()
            .find(|e| e.iso_639_1 == Some(code.as_str()))
            .map_or(Self::None, |e| e.variant)
    }

    /// Look up by ISO 639-2 code, matching either the /T or /B form (e.g.
    /// `"eng"`, `"ger"`). Case-insensitive and trimmed.
    pub fn from_iso_639_2(code: Option<&str>) -> Self {
        let Some(code) = normalize(code) else {
            return Self::None;
        };
        TABLE
            .iter()
            .find(|e| e.iso_639_2_t == Some(code.as_str()) || e.iso_639_2_b == Some(code.as_str()))
            .map_or(Self::None, |e| e.variant)
    }

    /// Look up by language name (English or native). Case-insensitive and
    /// trimmed.
    pub fn from_name(name: Option<&str>) -> Self {
        let Some(name) = normalize(name) else {
            return Self::None;
        };
        for e in TABLE {
            if e.name_en.is_some_and(|n| n.to_lowercase() == name) {
                return e.variant;
            }
            if e.name_native.is_some_and(|n| n.to_lowercase() == name) {
                return e.variant;
            }
        }
        Self::None
    }

    /// Flexible parse: matches an ISO 639-1, 639-2/T, or 639-2/B code, an
    /// English name, or a native name. Case-insensitive and trimmed. `"und"`
    /// and empty input yield [`LanguageCode::None`].
    pub fn from_string(value: Option<&str>) -> Self {
        let Some(value) = normalize(value) else {
            return Self::None;
        };
        if value == "und" {
            return Self::None;
        }
        let v = value.as_str();
        for e in TABLE {
            if e.iso_639_1 == Some(v)
                || e.iso_639_2_t == Some(v)
                || e.iso_639_2_b == Some(v)
                || e.name_en.is_some_and(|n| n.to_lowercase() == value)
                || e.name_native.is_some_and(|n| n.to_lowercase() == value)
            {
                return e.variant;
            }
        }
        Self::None
    }

    /// Whether a string represents a valid (non-`None`) language.
    pub fn is_valid_language(value: Option<&str>) -> bool {
        Self::from_string(value) != Self::None
    }

    /// Mirrors Python `bool(lang)`: true unless this is [`LanguageCode::None`].
    pub fn is_some(self) -> bool {
        self.to_iso_639_1().is_some()
    }

    /// Mirrors Python `str(lang)`: the English name, or `"Unknown"`.
    pub fn display_name(self) -> &'static str {
        self.name_en().unwrap_or("Unknown")
    }
}

impl std::fmt::Display for LanguageCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Lowercase + trim, returning `None` for empty/absent input. Matches the
/// Python guard `if not code: ...` followed by `code.lower().strip()`.
fn normalize(s: Option<&str>) -> Option<String> {
    let s = s?;
    if s.is_empty() {
        return None;
    }
    let normalized = s.trim().to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_101_entries() {
        assert_eq!(TABLE.len(), 101);
    }

    #[test]
    fn iso_639_2_b_divergences() {
        assert_eq!(LanguageCode::TIBETAN.to_iso_639_2_t(), Some("bod"));
        assert_eq!(LanguageCode::TIBETAN.to_iso_639_2_b(), Some("tib"));
        assert_eq!(LanguageCode::GERMAN.to_iso_639_2_b(), Some("ger"));
        assert_eq!(LanguageCode::CZECH.to_iso_639_2_b(), Some("cze"));
    }

    #[test]
    fn round_trips_and_none() {
        assert_eq!(LanguageCode::from_iso_639_1(Some("EN ")), LanguageCode::ENGLISH);
        assert_eq!(LanguageCode::from_iso_639_2(Some("ger")), LanguageCode::GERMAN);
        assert_eq!(LanguageCode::from_iso_639_2(Some("deu")), LanguageCode::GERMAN);
        assert_eq!(LanguageCode::from_string(Some("und")), LanguageCode::None);
        assert_eq!(LanguageCode::from_string(None), LanguageCode::None);
        assert_eq!(LanguageCode::None.display_name(), "Unknown");
        assert!(!LanguageCode::None.is_some());
        assert!(LanguageCode::ENGLISH.is_some());
    }
}
