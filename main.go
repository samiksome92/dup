/*
Dup finds duplicate files in a list of given directories and optionally deletes any duplicates found.

Usage:

	dup [flags] dir ...

The flags are:

	-x, --cross       Cross check across directories.
	-h, --help        Print this help.
	-r, --recursive   Recursively check files.

Dup only compares files which have the same size. The files are compared byte for byte and marked as duplicates if they
are same. Once all files are processed a table of duplicate files founds and their matches are displayed. The user is
then provided with an option for deleting all duplicates detected.
*/
package main

import (
	"bytes"
	"fmt"
	"io"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"sort"

	"github.com/fatih/color"
	"github.com/rodaine/table"
	"github.com/spf13/pflag"
)

// Number of bytes to read at once while comparing files.
const CHUNK_SIZE = 1024 * 1024

// listDir retrieves a list of all files in the directory, recursively traversing the tree if specified.
func listDir(dir string, recursive bool) []string {
	var files []string
	if recursive {
		// If recursive is specified use WalkDir to traverse the directory tree and collect all files.
		filepath.WalkDir(dir, func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				log.Fatal(err)
			}

			if !d.IsDir() {
				files = append(files, path)
			}

			return nil
		})
	} else {
		// Otherwise just use ReadDir to read files.
		f, err := os.Open(dir)
		if err != nil {
			log.Fatal(err)
		}

		ds, err := f.ReadDir(0)
		if err != nil {
			log.Fatal(err)
		}

		for _, d := range ds {
			if !d.IsDir() {
				files = append(files, filepath.Join(dir, d.Name()))
			}
		}

		f.Close()
	}

	return files
}

// makePairs generates pairs of files to compare against each other. If cross is specified files within the same
// directory are not compared.
func makePairs(files [][]string, cross bool) [][2]string {
	var pairs [][2]string

	for i := 0; i < len(files); i++ {
		// If cross is false compare within directory as well.
		if !cross {
			for x := 0; x < len(files[i]); x++ {
				for y := x + 1; y < len(files[i]); y++ {
					pairs = append(pairs, [2]string{files[i][x], files[i][y]})
				}
			}
		}

		for j := i + 1; j < len(files); j++ {
			for _, file1 := range files[i] {
				for _, file2 := range files[j] {
					pairs = append(pairs, [2]string{file1, file2})
				}
			}
		}
	}

	return pairs
}

// fileCmp reports whether two files are same byte for byte.
func fileCmp(file1 string, file2 string) bool {
	// Open both files and get their stats.
	f1, err := os.Open(file1)
	if err != nil {
		log.Fatal(err)
	}
	defer f1.Close()

	stat1, err := f1.Stat()
	if err != nil {
		log.Fatal(err)
	}

	f2, err := os.Open(file2)
	if err != nil {
		log.Fatal(err)
	}
	defer f2.Close()

	stat2, err := f2.Stat()
	if err != nil {
		log.Fatal(err)
	}

	// If files have different sizes they cannot be same.
	if stat1.Size() != stat2.Size() {
		f1.Close()
		f2.Close()

		return false
	}

	// Read bytes in chunks and compare them.
	b1 := make([]byte, CHUNK_SIZE)
	b2 := make([]byte, CHUNK_SIZE)
	for {
		n1, err1 := f1.Read(b1)
		n2, err2 := f2.Read(b2)

		if err1 == io.EOF && err2 == io.EOF {
			return true
		} else if err1 == io.EOF && err2 == nil {
			return false
		} else if err1 == nil && err2 == io.EOF {
			return false
		} else if err1 != nil || err2 != nil {
			log.Fatal(err1, err2)
		}

		if n1 != n2 {
			return false
		}

		if n1 < CHUNK_SIZE {
			b1 = b1[:CHUNK_SIZE]
			b2 = b2[:CHUNK_SIZE]
		}

		if !bytes.Equal(b1, b2) {
			return false
		}
	}
}

// findDups takes pairs of files and returns pairs of duplicate files.
func findDups(pairs [][2]string) [][2]string {
	dups := make(map[string]string)
	for _, pair := range pairs {
		file1, file2 := pair[0], pair[1]

		// If either file has already been marked as a duplicate skip this pair.
		_, ok1 := dups[file1]
		_, ok2 := dups[file2]
		if ok1 || ok2 {
			continue
		}

		// Else compare them and mark file2 as duplicate of file1 if needed.
		if fileCmp(pair[0], pair[1]) {
			dups[file2] = file1
		}
	}

	// Sort into a list of string pairs.
	files := make([]string, 0, len(dups))
	for f := range dups {
		files = append(files, f)
	}
	sort.Slice(files, func(i, j int) bool {
		return files[i] < files[j]
	})

	dupsSorted := make([][2]string, 0, len(files))
	for _, f := range files {
		dupsSorted = append(dupsSorted, [2]string{f, dups[f]})
	}

	return dupsSorted
}

func main() {
	log.SetFlags(0)

	// Define and parse command line arguments.
	help := pflag.BoolP("help", "h", false, "Print this help.")
	cross := pflag.BoolP("cross", "x", false, "Cross check across directories.")
	recursive := pflag.BoolP("recursive", "r", false, "Recursively check files.")
	pflag.Parse()

	// If --help is present print help and exit.
	if *help {
		fmt.Println("Usage: dup [flags] dir ...")
		pflag.PrintDefaults()
		os.Exit(0)
	}

	// Ensure arguments are valid.
	dirs := pflag.Args()
	if len(dirs) == 0 {
		fmt.Println("At least one directory is required.")
		os.Exit(1)
	} else if *cross && len(dirs) == 1 {
		fmt.Println("At least two directories are required for cross directory check.")
		os.Exit(1)
	}

	// Get list of files, generate pairs and find duplicates.
	var files [][]string
	for _, dir := range dirs {
		files = append(files, listDir(dir, *recursive))
	}
	pairs := makePairs(files, *cross)
	dups := findDups(pairs)

	// If no duplicates are found print so and exit.
	if len(dups) == 0 {
		fmt.Println("No duplicate files found.")
		os.Exit(0)
	}

	// Otherwise print a table of all duplicate files and their matches.
	fmt.Printf("Found %v duplicate files.\n", len(dups))

	tbl := table.New("File", "Matched to")
	tbl.WithHeaderFormatter(color.New(color.Italic).Add(color.Underline).SprintfFunc())
	for _, pair := range dups {
		tbl.AddRow(pair[0], pair[1])
	}
	tbl.Print()

	// Ask whether duplicate files are to be deleted.
	var delete rune
	fmt.Print("Delete duplicates? [y/N] ")
	fmt.Scanf("%c", &delete)
	if delete == 'y' || delete == 'Y' {
		fmt.Printf("Deleting %v files...\n", len(dups))
		for _, pair := range dups {
			err := os.Remove(pair[0])
			if err != nil {
				log.Println(err)
			}
		}
		fmt.Println("Done.")
	} else {
		fmt.Println("Not deleting.")
	}
}
