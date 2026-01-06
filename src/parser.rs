use crate::Graph;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, space0, space1},
    combinator::{map_res, opt},
    multi::{many1, separated_list1},
    sequence::{preceded, terminated},
    IResult,
};
use std::path::PathBuf;

/// Parse line ending (handles both \n and \r\n)
fn line_ending(input: &str) -> IResult<&str, &str> {
    alt((tag("\n"), tag("\r\n")))(input)
}

/// Parse a single unsigned integer
fn parse_usize(input: &str) -> IResult<&str, usize> {
    map_res(digit1, |s: &str| s.parse::<usize>())(input)
}

/// Parse a row of space-separated integers
fn parse_row(input: &str) -> IResult<&str, Vec<usize>> {
    preceded(space0, separated_list1(space1, parse_usize))(input)
}

/// Parse a complete adjacency matrix (n rows of n elements each)
fn parse_adjacency_matrix(input: &str, n: usize) -> IResult<&str, Vec<Vec<usize>>> {
    let mut rows = Vec::with_capacity(n);
    let mut remaining = input;

    for _ in 0..n {
        let (rest, row) = terminated(parse_row, opt(line_ending))(remaining)?;

        if row.len() != n {
            return Err(nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::LengthValue,
            )));
        }

        rows.push(row);
        remaining = rest;
    }

    Ok((remaining, rows))
}

/// Parse a single graph: vertex count followed by adjacency matrix
fn parse_graph(input: &str) -> IResult<&str, Graph> {
    // Parse vertex count
    let (input, n) = terminated(preceded(space0, parse_usize), line_ending)(input)?;

    // Parse adjacency matrix
    let (input, adj) = parse_adjacency_matrix(input, n)?;

    Ok((input, Graph::from_adjacency_matrix(adj)))
}

/// Parse two graphs from input string
pub fn parse_two_graphs(input: &str) -> IResult<&str, (Graph, Graph)> {
    let (input, g) = parse_graph(input)?;
    // Allow optional blank lines between graphs
    let (input, _) = opt(many1(line_ending))(input)?;
    let (input, h) = parse_graph(input)?;

    Ok((input, (g, h)))
}

/// Parse input file containing two graph descriptions
pub fn parse_input_file(path: &PathBuf) -> Result<(Graph, Graph), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;

    match parse_two_graphs(&content) {
        Ok((_, graphs)) => Ok(graphs),
        Err(e) => Err(format!("Parse error: {}", e).into()),
    }
}
