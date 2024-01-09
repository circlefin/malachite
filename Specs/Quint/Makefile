parse:
	npx @informalsystems/quint parse asyncModelsTest.qnt
	npx @informalsystems/quint parse consensusTest.qnt
	npx @informalsystems/quint parse driver.qnt
	npx @informalsystems/quint parse statemachineTest.qnt
	npx @informalsystems/quint parse votekeeperTest.qnt
.PHONY: parse

test:
	find *Test.qnt -maxdepth 1 -type f -exec time quint test {} \; 
.PHONY: test

verify:
	npx @informalsystems/quint verify --max-steps 10 \
		--main votekeeperModels --init=VK1::init --step=VK1::step \
		--invariant VK1::Inv \
		votekeeperModels.qnt
.PHONY: verify

# Generate traces by random simulation from properties describing the last desired state.
traces-sim:
	./scripts/gen-traces.sh votekeeperTest.qnt emitPrecommitValue 30
	./scripts/gen-traces.sh votekeeperTest.qnt emitPolkaAny 30
	./scripts/gen-traces.sh votekeeperTest.qnt emitPolkaNil 30
	./scripts/gen-traces.sh votekeeperTest.qnt emitSkip 30
.PHONY: traces-sim

# Generate traces from `run` statements.
traces-run-vk:
	mkdir -p traces/votekeeper
	npx @informalsystems/quint test --output traces/votekeeper/{}.itf.json votekeeperTest.qnt
.PHONY: traces-run-vk

traces-run-con:
	mkdir -p traces/consensus
	npx @informalsystems/quint test --output traces/consensus/{}.itf.json consensusTest.qnt
.PHONY: traces-run-con

traces-run: traces-run-vk traces-run-con
.PHONY: traces-run

traces: traces-sim traces-run
.PHONY: traces
