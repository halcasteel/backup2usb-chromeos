use anyhow::Result;
use std::path::Path;
use tokio::process::Command;

/// Run rsync with common options
pub async fn run_rsync(
    source: &Path,
    destination: &Path,
    excludes: &[String],
    dry_run: bool,
) -> Result<()> {
    let mut cmd = Command::new("rsync");
    
    // Basic options
    cmd.args(&["-avz", "--progress", "--stats"]);
    
    // Preserve permissions where possible
    cmd.args(&["--no-perms", "--no-owner", "--no-group"]);
    
    if dry_run {
        cmd.arg("--dry-run");
    }
    
    // Add excludes
    for exclude in excludes {
        cmd.arg(format!("--exclude={}", exclude));
    }
    
    // Source and destination
    cmd.arg(source);
    cmd.arg(destination);
    
    let output = cmd.output().await?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("rsync failed: {}", stderr));
    }
    
    Ok(())
}

/// Check if rsync is available
pub async fn check_rsync() -> Result<String> {
    let output = Command::new("rsync")
        .arg("--version")
        .output()
        .await?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("rsync not found"));
    }
    
    let version = String::from_utf8_lossy(&output.stdout);
    Ok(version.lines().next().unwrap_or("unknown").to_string())
}