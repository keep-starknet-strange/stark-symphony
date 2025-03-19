use anyhow::{Context, Result};
use base64::display::Base64Display;
use base64::engine::general_purpose::STANDARD;
use clap::{Parser, Subcommand};
use simfony::{Arguments, CompiledProgram, WitnessValues};
// use simfony::{dummy_env, simplicity::BitMachine};
use simplicity::ffi::tests::{run_program, TestUpTo};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "simfony")]
#[command(about = "Simfony language CLI tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a Simfony program
    Build {
        /// Path to the source file
        path: PathBuf,

        /// Path to the witness file
        #[arg(long)]
        witness: Option<PathBuf>,

        /// Path to write the compiled program
        #[arg(long, name = "output-path")]
        output_path: Option<PathBuf>,
    },

    /// Run a Simfony program
    Run {
        /// Path to the source file
        path: PathBuf,

        /// Path to the witness file
        #[arg(long)]
        witness: Option<PathBuf>,

        /// Path to file with arguments
        #[arg(long)]
        param: Option<PathBuf>,
    },
}

fn parse_witness(content: Option<&str>) -> Result<WitnessValues> {
    content.map_or(Ok(WitnessValues::default()), |s| {
        serde_json::from_str(s).with_context(|| "Failed to parse witness")
    })
}

fn parse_arguments(content: Option<&str>) -> Result<Arguments> {
    content.map_or(Ok(Arguments::default()), |s| {
        serde_json::from_str(s).with_context(|| "Failed to parse arguments")
    })
}

fn write_build_output(
    output_path: Option<PathBuf>,
    program_bytes: &[u8],
    witness_bytes: Option<&[u8]>,
) -> Result<()> {
    match output_path {
        Some(path) => {
            fs::write(&path, program_bytes)
                .with_context(|| format!("Failed to write output file: {}", path.display()))?;
        }
        None => {
            println!("Program:\n{}", Base64Display::new(program_bytes, &STANDARD));
            if let Some(witness) = witness_bytes {
                println!("Witness:\n{}", Base64Display::new(witness, &STANDARD));
            }
        }
    }
    Ok(())
}

fn handle_build(
    path: PathBuf,
    witness: Option<PathBuf>,
    output_path: Option<PathBuf>,
) -> Result<()> {
    let source = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read source file: {}", path.display()))?;

    let compiled = CompiledProgram::new(source, Arguments::default())
        .map_err(|e| anyhow::anyhow!(e))
        .with_context(|| "Failed to compile program")?;

    if let Some(witness_path) = witness {
        let witness_content = fs::read_to_string(&witness_path)
            .with_context(|| format!("Failed to read witness file: {}", witness_path.display()))?;

        let witness = parse_witness(Some(&witness_content))?;
        let satisfied = compiled
            .satisfy(witness)
            .map_err(|e| anyhow::anyhow!(e))
            .with_context(|| "Failed to satisfy witness")?;
        let (program_bytes, witness_bytes) = satisfied.redeem().encode_to_vec();

        write_build_output(output_path, &program_bytes, Some(&witness_bytes))
    } else {
        let program_bytes = compiled.commit().encode_to_vec();
        write_build_output(output_path, &program_bytes, None)
    }
}

fn handle_run(path: PathBuf, witness: Option<PathBuf>, param: Option<PathBuf>) -> Result<()> {
    let source = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read source file: {}", path.display()))?;

    let param_content =
        if let Some(param_path) = param {
            Some(fs::read_to_string(&param_path).with_context(|| {
                format!("Failed to read parameter file: {}", param_path.display())
            })?)
        } else {
            None
        };

    let witness_content =
        if let Some(witness_path) = witness {
            Some(fs::read_to_string(&witness_path).with_context(|| {
                format!("Failed to read witness file: {}", witness_path.display())
            })?)
        } else {
            None
        };

    let arguments = parse_arguments(param_content.as_deref())?;
    let compiled = CompiledProgram::new(source, arguments).map_err(|e| anyhow::anyhow!(e))?;
    let witness = parse_witness(witness_content.as_deref())?;
    let satisfied = compiled.satisfy(witness).map_err(|e| anyhow::anyhow!(e))?;

    let (program_bytes, witness_bytes) = satisfied.redeem().encode_to_vec();

    let _ = run_program(&program_bytes, &witness_bytes, TestUpTo::Everything)
        .map_err(|e| anyhow::anyhow!("Failed to run program: {}", e))?;

    // let mut machine = BitMachine::for_program(satisfied.redeem());
    // let env = dummy_env::dummy();
    // let res = machine.exec(satisfied.redeem(), &env)?;

    //println!("Result: {}", res);
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build {
            path,
            witness,
            output_path,
        } => handle_build(path, witness, output_path),
        Commands::Run {
            path,
            witness,
            param,
        } => handle_run(path, witness, param),
    };

    if let Err(err) = result {
        eprintln!("Error: {:#}", err);
        std::process::exit(1);
    }
}
