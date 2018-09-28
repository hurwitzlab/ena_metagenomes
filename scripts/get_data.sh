#!/bin/bash

CWD=$(cd $(dirname "$0") && pwd)
OUT_DIR="$CWD/../data"
URL="https://www.ebi.ac.uk/ena/data/view/Taxon:408172&portal=sample&display=xml"

[[ ! -d "$OUT_DIR" ]] && mkdir -p "$OUT_DIR"

cd "$OUT_DIR"

OUT_FILE="ena.xml"

if [[ ! -e "$OUT_FILE" ]]; then
    echo "Getting data"
    curl -o "$OUT_FILE" "$URL"
fi

echo "Splitting $OUT_FILE"
xml_split

echo "Done, see OUT_DIR \"$OUT_DIR\""
