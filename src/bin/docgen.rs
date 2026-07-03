use std::env;
use std::process::Command;

fn main() -> Result<(), String> {
	// Parse CLI arguments
	let args: Vec<String> = env::args().collect();
	let mut commit_msg = String::from("docs: update API documentation");
	let mut push = true;

	let mut i = 1;
	while i < args.len() {
		match args[i].as_str() {
			"-m" | "--message" => {
				if i + 1 < args.len() {
					commit_msg = args[i + 1].clone();
					i += 2;
				} else {
					return Err("Error: Missing value for --message option".to_string());
				}
			}
			"--no-push" => {
				push = false;
				i += 1;
			}
			"-h" | "--help" => {
				println!("docgen CLI - Generate documentation, commit, and push to git\n");
				println!("Usage:");
				println!("  cargo run --bin docgen [OPTIONS]\n");
				println!("Options:");
				println!("  -m, --message <MSG>   Set the git commit message (default: \"docs: update API documentation\")");
				println!("      --no-push         Skip pushing to the remote repository");
				println!("  -h, --help            Print help information");
				return Ok(());
			}
			arg => {
				return Err(format!(
					"Error: Unknown argument '{}'. Use --help for usage information.",
					arg
				));
			}
		}
	}

	println!("--- Step 1: Generating Cargo Docs ---");
	let status = Command::new("cargo")
		.args(["doc", "--no-deps"])
		.status()
		.map_err(|e| format!("Failed to run 'cargo doc': {e}"))?;

	if !status.success() {
		return Err("cargo doc execution failed".to_string());
	}
	println!("Documentation generated successfully.");

	println!("\n--- Step 2: Staging Git Changes ---");
	let status = Command::new("git")
		.args(["add", "."])
		.status()
		.map_err(|e| format!("Failed to run 'git add': {e}"))?;

	if !status.success() {
		return Err("git add execution failed".to_string());
	}

	// Verify if there are any changes staged for commit
	let output = Command::new("git")
		.args(["status", "--porcelain"])
		.output()
		.map_err(|e| format!("Failed to execute 'git status': {e}"))?;

	if output.stdout.is_empty() {
		println!("No changes detected. Everything is up to date.");
		return Ok(());
	}

	println!("\n--- Step 3: Committing Changes ---");
	let status = Command::new("git")
		.args(["commit", "-m", &commit_msg])
		.status()
		.map_err(|e| format!("Failed to run 'git commit': {e}"))?;

	if !status.success() {
		return Err("git commit execution failed".to_string());
	}

	if push {
		println!("\n--- Step 4: Pushing to Remote Repository ---");
		let status = Command::new("git")
			.args(["push"])
			.status()
			.map_err(|e| format!("Failed to run 'git push': {e}"))?;

		if !status.success() {
			return Err("git push execution failed".to_string());
		}
		println!("Successfully pushed changes to remote!");
	} else {
		println!("\nSkipping git push (--no-push).");
	}

	println!("\nDone! All steps completed successfully.");
	Ok(())
}
