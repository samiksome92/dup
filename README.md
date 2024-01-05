# Dup

Dup finds duplicate files in a list of given directories and optionally deletes any duplicates found.

## Installation
Either download a release directly from the [releases](https://github.com/samiksome92/dup/releases) page or use Go:

    go install github.com/samiksome92/dup@latest

## Usage
	dup [flags] dir ...

The flags are:

	-x, --cross       Cross check across directories.
	-h, --help        Print this help.
	-r, --recursive   Recursively check files.

Dup only compares files which have the same size. The files are compared byte for byte and marked as duplicates if they
are same. Once all files are processed a table of duplicate files founds and their matches are displayed. The user is
then provided with an option for deleting all duplicates detected.
