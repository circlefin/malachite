parse:
	npx @informalsystems/quint parse asyncModelsTest.qnt
	npx @informalsystems/quint parse consensusTest.qnt
	npx @informalsystems/quint parse executor.qnt
	npx @informalsystems/quint parse statemachineTest.qnt
	npx @informalsystems/quint parse voteBookkeeperTest.qnt
.PHONY: parse

test-con:
	npx @informalsystems/quint run consensusTest.qnt
.PHONY: test-con

test-sm:
	npx @informalsystems/quint test statemachineTest.qnt
.PHONY: test-sm

test-vk:
	npx @informalsystems/quint test voteBookkeeperTest.qnt
.PHONY: test-vk

test: test-con test-sm test-vk
.PHONY: test

verify:
	npx @informalsystems/quint verify --max-steps 10 --invariant Inv voteBookkeeperTest.qnt
.PHONY: verify

# Generate traces by random simulation from properties describing the last desired state.
traces-sim:
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPrecommitValue 30
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPolkaAny 30
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPolkaNil 30
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitSkip 30
.PHONY: traces-sim

# Generate traces following the test `run`s step by step.
traces-run:
	mkdir -p traces
	npx @informalsystems/quint test --output traces/voteBookkeeper_{}.itf.json voteBookkeeperTest.qnt
	rm traces/*voteBookkeeperTest::*.*
	find traces -maxdepth 1 -type f -exec ../../Scripts/trace-remove-stuttering-steps.sh {} \; 
	mkdir -p ../../Code/itf/tests/fixtures/votekeeper
	mv traces/* ../../Code/itf/tests/fixtures/votekeeper
	rm -rdf traces
.PHONY: traces-run

traces: traces-sim traces-run
.PHONY: traces
