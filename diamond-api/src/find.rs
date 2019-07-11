use actix_web::{
    AsyncResponder, Form, FromRequest, HttpMessage, HttpRequest, HttpResponse, Json, Query, State,
};
use failure::*;
use futures::future::{err, result, Future};
use glob::Pattern;
use serde::*;
use std::convert::From;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ParseError;
use crate::opts::*;
use crate::parse::de_time_parse;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MetricResponse {
    metrics: Vec<MetricResponseLeaf>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
struct MetricResponseLeaf {
    name: String,
    path: String,
    is_leaf: bool,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
struct JsonTreeLeaf {
    text: String,
    id: String,
    #[serde(rename = "allowChildren")]
    allow_children: u8,
    expandable: u8,
    leaf: u8,
}

impl From<MetricResponseLeaf> for JsonTreeLeaf {
    fn from(m: MetricResponseLeaf) -> JsonTreeLeaf {
        if m.is_leaf {
            JsonTreeLeaf {
                text: m.path,
                id: m.name,
                allow_children: 0,
                expandable: 0,
                leaf: 1,
            }
        } else {
            JsonTreeLeaf {
                text: m.path,
                id: m.name,
                allow_children: 1,
                expandable: 1,
                leaf: 0,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindFormat {
    TreeJson,
    Completer,
}

impl Default for FindFormat {
    fn default() -> FindFormat {
        FindFormat::TreeJson
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindQuery {
    query: String,
    #[serde(default)]
    format: FindFormat,
    #[serde(default)]
    wildcards: u8,
    #[serde(deserialize_with = "de_time_parse", default)]
    from: u32,
    #[serde(deserialize_with = "de_time_parse", default = "u32::max_value")]
    until: u32,
}

impl<S: 'static> FromRequest<S> for FindQuery {
    type Config = ();
    type Result = Result<Self, actix_web::error::Error>;

    fn from_request(req: &HttpRequest<S>, _cfg: &Self::Config) -> Self::Result {
        match req.content_type().to_lowercase().as_str() {
            "application/x-www-form-urlencoded" => {
                Ok(Form::<FindQuery>::extract(req).wait()?.into_inner())
            }
            "application/json" => Ok(Json::<FindQuery>::extract(req).wait()?.into_inner()),
            _ => Ok(Query::<FindQuery>::extract(req)?.into_inner()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct FindPath {
    path: PathBuf,
    pattern: Pattern,
}

impl FindPath {
    fn from(query: &FindQuery) -> Result<FindPath, ParseError> {
        let s = &query.query;
        let mut segments: Vec<&str> = s.split('.').collect();

        let (path, pattern) = match segments.len() {
            len if len > 1 => {
                let pattern = segments.pop().ok_or(ParseError::Unknown)?;
                let path = segments.iter().fold(PathBuf::new(), |acc, x| acc.join(x));
                (path, pattern)
            }
            1 => (PathBuf::from(""), s.as_str()),
            _ => return Err(ParseError::Unknown),
        };

        let pattern_str = match query.wildcards {
            0 => pattern.to_owned(),
            1 => [&pattern, "*"].concat(),
            _ => pattern.to_owned(),
        };

        Ok(FindPath {
            path,
            pattern: Pattern::new(&pattern_str)?,
        })
    }
}

fn create_leaf(name: &str, dir: &str, is_leaf: bool) -> MetricResponseLeaf {
    let path = if !dir.is_empty() {
        format!("{}.{}", dir, name)
    } else {
        name.to_owned()
    };

    MetricResponseLeaf {
        name: name.to_owned(),
        path,
        is_leaf,
    }
}

#[inline]
fn walk_tree(
    dir: &Path,
    subdir: &Path,
    pattern: &Pattern,
) -> Result<Vec<MetricResponseLeaf>, Error> {
    let full_path = dir.canonicalize()?.join(&subdir);
    let dir_metric = subdir
        .components()
        .filter_map(|x| x.as_os_str().to_str())
        .collect::<Vec<&str>>()
        .concat();

    let mut metrics: Vec<MetricResponseLeaf> = fs::read_dir(&full_path)?
        .filter_map(|entry| {
            let (local_path, local_file_type) = match entry {
                Ok(rentry) => (
                    rentry
                        .path()
                        .strip_prefix(&full_path)
                        .map(std::borrow::ToOwned::to_owned),
                    rentry.file_type(),
                ),
                _ => return None,
            };

            match (&local_path, &local_file_type) {
                (Ok(path), Ok(file_type)) if pattern.matches_path(&path) && file_type.is_dir() => {
                    let name = match path.file_name() {
                        Some(file_name) => {
                            if let Some(file_name_str) = file_name.to_str() {
                                file_name_str.to_owned()
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    };
                    Some(create_leaf(&name, &dir_metric, false))
                }
                (Ok(path), Ok(file_type))
                    if pattern.matches_path(&path)
                        && file_type.is_file()
                        && path.extension() == Some(OsStr::new("wsp")) =>
                {
                    let name = match path.file_stem() {
                        Some(file_name) => {
                            if let Some(file_name_str) = file_name.to_str() {
                                file_name_str.to_owned()
                            } else {
                                return None;
                            }
                        }
                        _ => return None,
                    };

                    Some(create_leaf(&name, &dir_metric, true))
                }
                _ => None,
            }
        })
        .collect();

    metrics.sort_by_key(|k| k.name.clone());
    Ok(metrics)
}

pub fn find_handler(
    args: State<Args>,
    params: FindQuery,
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    match FindPath::from(&params) {
        Ok(path) => match walk_tree(&args.path, &path.path, &path.pattern) {
            Ok(metrics) => {
                if params.format == FindFormat::TreeJson {
                    let metrics_json: Vec<JsonTreeLeaf> = metrics
                        .iter()
                        .map(|x| JsonTreeLeaf::from(x.to_owned()))
                        .collect();
                    result(Ok(HttpResponse::Ok().json(metrics_json))).responder()
                } else {
                    let metrics_completer = MetricResponse { metrics };
                    result(Ok(HttpResponse::Ok().json(metrics_completer))).responder()
                }
            }
            Err(e) => Box::new(err(e)),
        },
        Err(e) => Box::new(err(e.into())),
    }
}

#[cfg(test)]
mod tests {
    use serde_urlencoded;
    use tempfile;

    use super::*;
    use actix_web::test::TestRequest;
    use std::fs::create_dir;
    use std::fs::File;
    use std::path::{Path, PathBuf};

    fn get_temp_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("diamond-api")
            .tempdir()
            .expect("Temp dir created")
    }

    #[test]
    fn walk_tree_verify() -> Result<(), Error> {
        let dir = get_temp_dir();
        let path = dir.path();
        let path1 = path.join(Path::new("foo"));
        let path2 = path.join(Path::new("bar"));
        let path3 = path.join(Path::new("foobar.wsp"));
        let path4 = path1.join(Path::new("bar.wsp"));

        create_dir(&path1)?;
        create_dir(&path2)?;
        let _file1 = File::create(&path3).unwrap();
        let _file2 = File::create(&path4).unwrap();

        let metric = walk_tree(&path, &Path::new(""), &Pattern::new("*")?)?;

        let mut metric_cmp = vec![
            MetricResponseLeaf {
                name: "foo".into(),
                path: "foo".into(),
                is_leaf: false,
            },
            MetricResponseLeaf {
                name: "bar".into(),
                path: "bar".into(),
                is_leaf: false,
            },
            MetricResponseLeaf {
                name: "foobar".into(),
                path: "foobar".into(),
                is_leaf: true,
            },
        ];

        metric_cmp.sort_by_key(|k| k.name.clone());

        assert_eq!(metric, metric_cmp);

        let metric2 = walk_tree(&path, &Path::new("foo"), &Pattern::new("*")?)?;

        let mut metric_cmp2 = vec![MetricResponseLeaf {
            name: "bar".into(),
            path: "foo.bar".into(),
            is_leaf: true,
        }];

        metric_cmp2.sort_by_key(|k| k.name.clone());

        assert_eq!(metric2, metric_cmp2);

        Ok(())
    }

    #[test]
    fn url_serialize() -> Result<(), Error> {
        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::TreeJson,
            wildcards: 1,
            from: 0,
            until: 10,
        };

        assert_eq!(
            "query=123&format=treejson&wildcards=1&from=0&until=10",
            serde_urlencoded::to_string(params.clone())?
        );

        let params2 = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::Completer,
            wildcards: 0,
            from: 0,
            until: 10,
        };

        assert_eq!(
            "query=123&format=completer&wildcards=0&from=0&until=10",
            serde_urlencoded::to_string(params2.clone())?
        );

        Ok(())
    }

    #[test]
    fn url_deserialize() -> Result<(), Error> {
        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::TreeJson,
            wildcards: 1,
            from: 0,
            until: 10,
        };

        assert_eq!(
            serde_urlencoded::from_str("query=123&format=treejson&wildcards=1&from=0&until=10"),
            Ok(params)
        );

        let params2 = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::Completer,
            wildcards: 0,
            from: 0,
            until: 10,
        };

        assert_eq!(
            serde_urlencoded::from_str("query=123&format=completer&wildcards=0&from=0&until=10"),
            Ok(params2)
        );

        Ok(())
    }

    #[test]
    fn url_deserialize_default() -> Result<(), Error> {
        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::default(),
            wildcards: u8::default(),
            from: u32::default(),
            until: u32::max_value(),
        };

        assert_eq!(serde_urlencoded::from_str("query=123"), Ok(params));

        Ok(())
    }

    #[test]
    fn findpath_convert() -> Result<(), Error> {
        let path = FindPath {
            path: PathBuf::from("123/456/"),
            pattern: Pattern::new("789")?,
        };

        let params = FindQuery {
            query: "123.456.789".to_owned(),
            format: FindFormat::default(),
            wildcards: u8::default(),
            from: 0,
            until: 10,
        };

        assert_eq!(FindPath::from(&params)?, path);

        Ok(())
    }

    #[test]
    fn query_convertion() -> Result<(), Error> {
        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::TreeJson,
            wildcards: 1,
            from: 0,
            until: 10,
        };

        let path = FindPath {
            path: PathBuf::from(""),
            pattern: Pattern::new("123*")?,
        };

        assert_eq!(FindPath::from(&params)?, path);

        Ok(())
    }

    #[test]
    fn metric_response_convertion() {
        let mleaf: JsonTreeLeaf = MetricResponseLeaf {
            name: "456".to_owned(),
            path: "123.456".to_owned(),
            is_leaf: true,
        }
        .into();

        let leaf = JsonTreeLeaf {
            text: "123.456".to_owned(),
            id: "456".to_owned(),
            allow_children: 0,
            expandable: 0,
            leaf: 1,
        };

        assert_eq!(mleaf, leaf);

        let mleaf2: JsonTreeLeaf = MetricResponseLeaf {
            name: "789".to_owned(),
            path: "123.456.789".to_owned(),
            is_leaf: false,
        }
        .into();

        let leaf2 = JsonTreeLeaf {
            text: "123.456.789".to_owned(),
            id: "789".to_owned(),
            allow_children: 1,
            expandable: 1,
            leaf: 0,
        };

        assert_eq!(mleaf2, leaf2);
    }

    #[test]
    fn find_request_parse_url() -> Result<(), actix_web::error::Error> {
        let r =
            TestRequest::with_uri("/find?query=123&format=treejson&wildcards=1&from=0&until=10")
                .finish();

        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::TreeJson,
            wildcards: 1,
            from: 0,
            until: 10,
        };

        assert_eq!(FindQuery::from_request(&r, &())?, params);
        Ok(())
    }

    #[test]
    fn find_request_parse_form() -> Result<(), actix_web::error::Error> {
        let r = TestRequest::with_uri("/find")
            .header("content-type", "application/x-www-form-urlencoded")
            .set_payload("query=123&format=treejson&wildcards=1&from=0&until=10")
            .finish();

        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::TreeJson,
            wildcards: 1,
            from: 0,
            until: 10,
        };

        assert_eq!(FindQuery::from_request(&r, &())?, params);
        Ok(())
    }

    #[test]
    fn render_request_parse_json() -> Result<(), actix_web::error::Error> {
        let r = TestRequest::with_uri("/render")
            .header("content-type", "application/json")
            .set_payload(
                r#"{"query":"123","format":"treejson","wildcards":1,"from":"0","until":"10"}"#,
            )
            .finish();

        let params = FindQuery {
            query: "123".to_owned(),
            format: FindFormat::TreeJson,
            wildcards: 1,
            from: 0,
            until: 10,
        };

        assert_eq!(FindQuery::from_request(&r, &())?, params);
        Ok(())
    }
}
