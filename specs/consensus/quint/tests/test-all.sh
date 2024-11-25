#!/bin/bash
#
# Accepts optional parameters to `quint`, default: `--max-samples 100`.

QUINT_PARAMS=${@-"--max-samples 100"}

for TEST_FILE in */*Test.qnt
do
	quint test $QUINT_PARAMS $TEST_FILE
done 
