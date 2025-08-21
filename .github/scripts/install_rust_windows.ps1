# Download rustup-init
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe

# Install Rust
.\rustup-init.exe -y --default-toolchain stable --profile minimal

# Verify installation
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
rustc --version

# Clean up
Remove-Item rustup-init.exe
