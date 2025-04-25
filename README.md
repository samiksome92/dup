# dup
A command line utility to detect duplicate files present in directories.

## Installation
Either download a release directly from the [releases](https://github.com/samiksome92/dup/releases) page or use `cargo`:

    cargo install --git https://github.com/samiksome92/dup

## Usage
	dup [OPTIONS] <DIRS>...

Arguments:

	<DIRS>...  Directory(s)

Options:

	-x, --cross      Cross check across directories
	-r, --recursive  Recursively check directories
	-h, --help       Print help
	-V, --version    Print version

If `--cross` is specified only files across directories will be compared, otherwise files within a directory will also be compared with each other. `--recursive` recursively searches for files in directories. Once all files are processed a table of duplicate files founds and their matches are displayed. The user is then provided with an option for deleting all duplicates detected.
