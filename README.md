# th-commit

A simple CLI tool that creates git commits with AI-generated messages using the Theater runtime.

## Overview

`th-commit` is a streamlined git commit tool that:

1. Automatically detects your current git repository
2. Starts a Theater actor to handle the commit process
3. Shows real-time progress during the commit operation
4. Uses AI (Google's Gemini model) to generate meaningful commit messages

## Installation

```bash
# Clone the repository
git clone <repository-url>

# Build the tool
cd th-commit
cargo build --release

# Create a symbolic link (optional)
ln -s $(pwd)/target/release/th-commit /usr/local/bin/th-commit
```

## Usage

Simply run the command in any git repository:

```bash
th-commit
```

### Environment Variables

- `GOOGLE_GEMINI_API_KEY` (required): Your Google Gemini API key
- `THEATER_SERVER_ADDRESS` (optional): Address of the Theater server (default: 127.0.0.1:9000)

## Requirements

- Rust 1.70+
- A running Theater server
- Git repository
- Google Gemini API key

## How It Works

Behind the scenes, `th-commit`:

1. Connects to a Theater server
2. Starts the commit-actor with your repository path
3. The actor stages all changes
4. The actor generates a descriptive commit message using Gemini
5. The actor creates and pushes the commit
6. The CLI shows real-time progress and results

## License

[MIT License](LICENSE)
