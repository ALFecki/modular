use anyhow::anyhow;
use nom::branch::alt;
use nom::bytes::complete::{escaped_transform, is_not, tag};
use nom::character::complete::{alpha1, alphanumeric1, one_of};
use nom::combinator::{eof, map, opt, recognize};
use nom::multi::{many0, many0_count};
use nom::sequence::{pair, preceded, tuple};
use nom::IResult;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct Pattern {
    nodes: Vec<Node>,
    is_trailing_any: bool,
}

impl Display for Pattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut out = Vec::with_capacity(self.nodes.len() + 1);
        for node in &self.nodes {
            match node {
                Node::Arg(Some(v)) => out.push(format!("{{{}}}", v)),
                Node::Arg(None) => out.push("{}".to_owned()),
                Node::Const(v) => out.push(v.to_owned()),
            }
        }

        if self.is_trailing_any {
            out.push(">".to_owned());
        }

        write!(f, "{}", out.join("."))
    }
}

impl Pattern {
    pub fn parse<S: AsRef<str>>(str: S) -> anyhow::Result<Self> {
        let str = str.as_ref();

        let (_, (nodes, is_trailing_any)) =
            parse(str).map_err(|e| anyhow!("invalid pattern: {:?}", e))?;

        Ok(Self {
            nodes,
            is_trailing_any,
        })
    }

    pub fn matches<S: AsRef<str>>(&self, str: S) -> bool {
        let mut nodes_iter = self.nodes.iter();
        let other_iter = str.as_ref().split('.');

        for other_node in other_iter {
            match nodes_iter.next() {
                Some(v) => match v {
                    Node::Arg(_) => continue,
                    Node::Const(v) => {
                        if v != other_node {
                            return false;
                        }
                    }
                },
                None => {
                    return self.is_trailing_any;
                }
            }
        }

        true
    }
}

#[derive(Debug, Clone)]
enum Node {
    Arg(Option<String>),
    Const(String),
}

fn parse(str: &str) -> IResult<&str, (Vec<Node>, bool)> {
    let (rest, (mut start_nodes, is_trailing)) = alt((
        map(trailing_any, |is_trailing_any| (vec![], is_trailing_any)),
        map(node, |e| (vec![e], false)),
    ))(str)?;

    if is_trailing {
        return Ok((rest, (start_nodes, is_trailing)));
    }

    let (rest, nodes) = many0(preceded(tag("."), node))(rest)?;
    let (rest, is_trailing_any) = trailing_any(rest)?;

    start_nodes.extend(nodes);

    Ok((rest, (start_nodes, is_trailing_any)))
}

fn node(str: &str) -> IResult<&str, Node> {
    alt((const_node, arg_node))(str).map(|(s, node)| (s, node))
}

fn const_node(str: &str) -> IResult<&str, Node> {
    escaped_node(str).map(|(s, v)| (s, Node::Const(v)))
}

#[rustfmt::skip]
fn escaped_node(str: &str) -> IResult<&str, String> {
    if str.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            str,
            nom::error::ErrorKind::Eof,
        )));
    }
    
    escaped_transform(
        is_not("{}.\\>"), 
        '\\', 
        one_of("{}.\\>")
    )(str)
}

#[rustfmt::skip]
fn arg_node(str: &str) -> IResult<&str, Node> {
    pub fn identifier(input: &str) -> IResult<&str, &str> {
        recognize(
            pair(
                alt((alpha1, tag("_"))),
                many0_count(alt((alphanumeric1, tag("_"))))
            )
        )(input)
    }
    
    tuple((
        tag("{"),
        opt(identifier),
        tag("}"),
    ))(str)
    .map(|(s, (_, arg, _))| (s, Node::Arg(arg.map(|i| i.to_string()))))
}

#[rustfmt::skip]
fn trailing_any(str: &str) -> IResult<&str, bool> {
    tuple((opt(tag(".>")), eof))(str)
        .map(|(v, out)| (v, out.0.is_some()))
}
