use std::process::Command;

#[derive(Debug, Clone)]
pub struct Environment {
    git_hash: String,
}

impl Environment {
    pub fn git_hash(&self) -> &String {
        &self.git_hash
    }
}

fn get_git_hash() -> Result<String, Box<::std::error::Error>> {
    let output = Command::new("git").arg("rev-parse").arg("HEAD").output()?;

    let mut combined_output = String::with_capacity(output.stderr.len() + output.stdout.len());
    combined_output.push_str(&String::from_utf8_lossy(&output.stderr));
    combined_output.push_str(&String::from_utf8_lossy(&output.stdout));

    Ok(combined_output.trim().to_string())
}

pub fn get_environment() -> Result<Environment, Box<::std::error::Error>> {
    Ok(Environment {
        git_hash: get_git_hash()?,
    })
}
