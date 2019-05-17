use actix_web::{AsyncResponder, FromRequest, HttpMessage, HttpRequest, HttpResponse, Json, State};
use failure::*;
use futures::future::{err, result, Future};
use serde::*;
use std::iter::successors;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use whisper::interval::Interval;
use whisper::{ArchiveData, WhisperFile};

use crate::error::ParseError;
use crate::opts::*;
use crate::parse::{de_time_parse, time_parse};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RenderQuery {
    target: Vec<String>,
    format: RenderFormat,
    #[serde(deserialize_with = "de_time_parse")]
    from: u32,
    #[serde(deserialize_with = "de_time_parse")]
    until: u32,
}

impl FromStr for RenderQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw: Vec<(String, String)> = serde_urlencoded::from_str(s)?;

        let mut q = RenderQuery::default();
        for (key, value) in raw {
            match key.as_str() {
                "target" => q.target.push(value),
                "format" => q.format = value.parse()?,
                "from" => q.from = time_parse(value)?,
                "until" => q.until = time_parse(value)?,
                _ => {}
            };
        }

        Ok(q)
    }
}

impl<S: 'static> FromRequest<S> for RenderQuery {
    type Config = ();
    type Result = Result<Self, actix_web::error::Error>;

    fn from_request(req: &HttpRequest<S>, _cfg: &Self::Config) -> Self::Result {
        match req.content_type().to_lowercase().as_str() {
            "application/x-www-form-urlencoded" => Ok(String::extract(req)?.wait()?.parse()?),
            "application/json" => Ok(Json::<RenderQuery>::extract(req).wait()?.into_inner()),
            _ => Ok(req.query_string().parse()?),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderFormat {
    Png,
    Raw,
    Csv,
    Json,
    Svg,
    Pdf,
    Dygraph,
    Rickshaw,
}

impl Default for RenderFormat {
    fn default() -> Self {
        RenderFormat::Json
    }
}

impl FromStr for RenderFormat {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "png" => Ok(RenderFormat::Png),
            "raw" => Ok(RenderFormat::Raw),
            "csv" => Ok(RenderFormat::Csv),
            "json" => Ok(RenderFormat::Json),
            "svg" => Ok(RenderFormat::Svg),
            "pdf" => Ok(RenderFormat::Pdf),
            "dygraph" => Ok(RenderFormat::Dygraph),
            "rickshaw" => Ok(RenderFormat::Rickshaw),
            _ => Err(ParseError::RenderFormat),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderMetric(String);

#[derive(Debug, PartialEq)]
pub struct RenderPath(PathBuf);

#[inline]
fn path(dir: &Path, metric: &str) -> Result<PathBuf, Error> {
    let path = metric
        .split('.')
        .fold(PathBuf::new(), |acc, x| acc.join(x))
        .with_extension("wsp");
    let full_path = dir.canonicalize()?.join(&path);
    Ok(full_path)
}

fn walk(dir: &Path, metric: &str, interval: Interval) -> Result<Vec<RenderPoint>, Error> {
    let full_path = path(dir, metric)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;

    let ArchiveData {
        from_interval,
        step,
        values,
        ..
    } = WhisperFile::open(&full_path)?.fetch_auto_points(interval, now)?;

    let timestamps = successors(Some(from_interval), |i| i.checked_add(step));
    let points = values
        .into_iter()
        .zip(timestamps)
        .map(|(value, time)| RenderPoint(value, time))
        .collect();

    Ok(points)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderPoint(Option<f64>, u32);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderResponceEntry {
    target: String,
    datapoints: Vec<RenderPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderResponce {
    entries: Vec<Option<RenderResponceEntry>>,
}

pub fn render_handler(
    state: State<Args>,
    params: RenderQuery,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let dir = &state.path;

    match Interval::new(params.from, params.until).map_err(err_msg) {
        Ok(interval) => {
            let response: Result<Vec<RenderResponceEntry>, Error> = params
                .target
                .into_iter()
                .map(|x| {
                    Ok(RenderResponceEntry {
                        datapoints: walk(&dir, &x, interval)?,
                        target: x,
                    })
                })
                .collect();

            match response {
                Ok(response) => result(Ok(HttpResponse::Ok().json(response))).responder(),
                Err(e) => Box::new(err(e)),
            }
        }
        Err(e) => Box::new(err(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use failure::Error;

    #[test]
    fn url_deserialize_one() -> Result<(), Error> {
        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["app.numUsers".to_owned()].to_vec(),
            from: 0,
            until: 10,
        };

        assert_eq!(
            "target=app.numUsers&format=json&from=0&until=10".parse::<RenderQuery>()?,
            params
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_multiple() -> Result<(), Error> {
        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["app.numUsers".to_owned(), "app.numServers".to_owned()].to_vec(),
            from: 0,
            until: 10,
        };

        assert_eq!(
            "target=app.numUsers&target=app.numServers&format=json&from=0&until=10"
                .parse::<RenderQuery>()?,
            params
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_none() -> Result<(), Error> {
        let params = RenderQuery {
            format: RenderFormat::Json,
            target: Vec::new(),
            from: 0,
            until: 10,
        };

        assert_eq!(
            "format=json&from=0&until=10".parse::<RenderQuery>()?,
            params
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_other() -> Result<(), Error> {
        let params = RenderQuery {
            format: RenderFormat::Json,
            target: vec!["m1".to_owned()],
            from: 0,
            until: 10,
        };

        assert_eq!(
            "target=m1&format=json&from=0&until=10&other=x".parse::<RenderQuery>()?,
            params
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_time_yesterday_now() -> Result<(), Error> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;

        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["m1".to_owned(), "m2".to_owned()].to_vec(),
            from: now - 3600 * 24,
            until: now,
        };

        assert_eq!(
            "target=m1&target=m2&format=json&from=yesterday&until=now".parse::<RenderQuery>()?,
            params
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_time_relative() -> Result<(), Error> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as u32;

        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["m1".to_owned(), "m2".to_owned()].to_vec(),
            from: now - 5 * 3600 * 24,
            until: now - 5 * 60,
        };

        assert_eq!(
            "target=m1&target=m2&format=json&from=-5d&until=-5min".parse::<RenderQuery>()?,
            params
        );

        let params2 = RenderQuery {
            format: RenderFormat::Json,
            target: ["m1".to_owned(), "m2".to_owned()].to_vec(),
            from: now - 5 * 3600 * 24 * 7,
            until: now - 5,
        };

        assert_eq!(
            "target=m1&target=m2&format=json&from=-5w&until=-5s".parse::<RenderQuery>()?,
            params2
        );

        let params3 = RenderQuery {
            format: RenderFormat::Json,
            target: ["m1".to_owned(), "m2".to_owned()].to_vec(),
            from: now - 2 * 3600 * 24 * 365,
            until: now - 5 * 3600,
        };

        assert_eq!(
            "target=m1&target=m2&format=json&from=-2y&until=-5h".parse::<RenderQuery>()?,
            params3
        );

        let params4 = RenderQuery {
            format: RenderFormat::Json,
            target: ["m1".to_owned(), "m2".to_owned()].to_vec(),
            from: now - 5 * 3600 * 24 * 30,
            until: now - 60,
        };

        assert_eq!(
            "target=m1&target=m2&format=json&from=-5mon&until=-1min".parse::<RenderQuery>()?,
            params4
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_time_fail() -> Result<(), Error> {
        assert!("target=m1&target=m2&format=json&from=-&until=now"
            .parse::<RenderQuery>()
            .is_err());

        assert!("target=m1&target=m2&format=json&from=-d&until=now"
            .parse::<RenderQuery>()
            .is_err());

        assert!("target=m1&target=m2&format=json&from=&until=now"
            .parse::<RenderQuery>()
            .is_err());

        assert!("target=m1&target=m2&format=json&from=tomorrow&until=now"
            .parse::<RenderQuery>()
            .is_err());

        Ok(())
    }

    #[test]
    fn render_response_json() -> Result<(), Error> {
        let rd = serde_json::to_string(&[RenderResponceEntry {
            target: "entries".into(),
            datapoints: vec![
                RenderPoint(Some(1.0), 1_311_836_008),
                RenderPoint(Some(2.0), 1_311_836_009),
                RenderPoint(Some(3.0), 1_311_836_010),
                RenderPoint(Some(5.0), 1_311_836_011),
                RenderPoint(Some(6.0), 1_311_836_012),
                RenderPoint(None, 1_311_836_013),
            ],
        }])?;

        let rs =
            r#"[{"target":"entries","datapoints":[[1.0,1311836008],[2.0,1311836009],[3.0,1311836010],[5.0,1311836011],[6.0,1311836012],[null,1311836013]]}]"#;

        assert_eq!(rd, rs);
        Ok(())
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn render_response_json_parse() -> Result<(), Error> {
        let rd = [RenderResponceEntry {
            target: "entries".into(),
            datapoints: [
                RenderPoint(Some(1.0), 1_311_836_008),
                RenderPoint(Some(2.0), 1_311_836_009),
                RenderPoint(Some(3.0), 1_311_836_010),
                RenderPoint(Some(5.0), 1_311_836_011),
                RenderPoint(Some(6.0), 1_311_836_012),
                RenderPoint(None, 1_311_836_013),
            ]
            .to_vec(),
        }]
        .to_vec();

        let rs: Vec<RenderResponceEntry> = serde_json::from_str(r#"[{"target":"entries","datapoints":[[1.0,1311836008],[2.0,1311836009],[3.0,1311836010],[5.0,1311836011],[6.0,1311836012],[null,1311836013]]}]"#)?;

        assert_eq!(rd, rs);
        Ok(())
    }

    #[test]
    fn format_parse() {
        assert_eq!("png".parse(), Ok(RenderFormat::Png));
        assert_eq!("raw".parse(), Ok(RenderFormat::Raw));
        assert_eq!("csv".parse(), Ok(RenderFormat::Csv));
        assert_eq!("json".parse(), Ok(RenderFormat::Json));
        assert_eq!("svg".parse(), Ok(RenderFormat::Svg));
        assert_eq!("pdf".parse(), Ok(RenderFormat::Pdf));
        assert_eq!("dygraph".parse(), Ok(RenderFormat::Dygraph));
        assert_eq!("rickshaw".parse(), Ok(RenderFormat::Rickshaw));
        assert_eq!("".parse::<RenderFormat>(), Err(ParseError::RenderFormat));
    }

    #[test]
    fn render_request_parse_url() -> Result<(), actix_web::error::Error> {
        let r = TestRequest::with_uri("/render?target=app.numUsers&format=json&from=0&until=10")
            .finish();

        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["app.numUsers".to_owned()].to_vec(),
            from: 0,
            until: 10,
        };

        assert_eq!(RenderQuery::from_request(&r, &())?, params);
        Ok(())
    }

    #[test]
    fn render_request_parse_form() -> Result<(), actix_web::error::Error> {
        let r = TestRequest::with_uri("/render")
            .header("content-type", "application/x-www-form-urlencoded")
            .set_payload("target=app.numUsers&format=json&from=0&until=10")
            .finish();

        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["app.numUsers".to_owned()].to_vec(),
            from: 0,
            until: 10,
        };

        assert_eq!(RenderQuery::from_request(&r, &())?, params);
        Ok(())
    }

    #[test]
    fn render_request_parse_json() -> Result<(), actix_web::error::Error> {
        let r = TestRequest::with_uri("/render")
            .header("content-type", "application/json")
            .set_payload(r#"{ "target":["app.numUsers"],"format":"json","from":"0","until":"10"}"#)
            .finish();

        let params = RenderQuery {
            format: RenderFormat::Json,
            target: ["app.numUsers".to_owned()].to_vec(),
            from: 0,
            until: 10,
        };

        assert_eq!(RenderQuery::from_request(&r, &())?, params);
        Ok(())
    }
}
