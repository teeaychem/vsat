#![allow(dead_code)]

use clap::Parser;
use std::fs;
mod io;
mod procedures;
mod structures;

use crate::structures::solve::{Solve, SolveConfig, SolveResult};
use crate::structures::Formula;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// file to parse
    #[arg(short, long)]
    file: String,

    /// Print core on unsat
    #[arg(short, long, default_value_t = false)]
    core: bool,
}

fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    let args = Args::parse();

    let config = SolveConfig {
        core: args.core,
        analysis: 3,
    };

    if let Ok(contents) = fs::read_to_string(args.file) {
        if let Ok(formula) = Formula::from_dimacs(&contents) {
            let mut the_solve = Solve::from_formula(&formula, config);

            let result = the_solve.implication_solve();
            match result {
                Ok(SolveResult::Unsatisfiable) => {
                    println!("s UNSATISFIABLE");
                    std::process::exit(00);
                }
                Ok(SolveResult::Satisfiable) => {
                    println!("s SATISFIABLE");
                    std::process::exit(10);
                }
                Ok(SolveResult::Unkown) => {
                    println!("s Unkown");
                    std::process::exit(20);
                }
                _ => panic!("Solve error"),
            }
            // dbg!(&the_solve);
        }
    }
}
