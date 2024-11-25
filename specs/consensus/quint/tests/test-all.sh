#!/bin/bash
#
# Accepts optional parameters to `quint`, e.g., `--max-samples 100`.

QUINT_PARAMS=$@

for TEST_FILE in */*Test.qnt
do
	quint test $QUINT_PARAMS $TEST_FILE
done 
