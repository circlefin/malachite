parse:
	ls -1 ./specs/*.qnt | ../../../scripts/quint-forall.sh parse
.PHONY: parse

test:
	ls -1 ./tests/**/*Test.qnt | ../../../scripts/quint-forall.sh test --max-samples=1000
.PHONY: test

verify:
	npx @informalsystems/quint verify --max-steps 10 \
		--main votekeeperModels --init=VK1::init --step=VK1::step \
		--invariant VK1::Inv \
		./tests/votekeeper/votekeeperModels.qnt
.PHONY: verify

# Generate traces by random simulation from properties describing the last desired state.
# FIXME: at the moment no useful traces are produced, even with longer executions
traces-sim:
	./scripts/gen-traces.sh tests/votekeeper/votekeeperModels.qnt emitPrecommitValue 30
	./scripts/gen-traces.sh tests/votekeeper/votekeeperModels.qnt emitPolkaAny 30
	./scripts/gen-traces.sh tests/votekeeper/votekeeperModels.qnt emitPolkaNil 30
	./scripts/gen-traces.sh tests/votekeeper/votekeeperModels.qnt emitSkip 30
.PHONY: traces-sim

# Generate traces from `run` statements.
traces-run-vk:
	mkdir -p traces/votekeeper
	npx @informalsystems/quint test --output traces/votekeeper/{}.itf.json ./tests/votekeeper/votekeeperTest.qnt
.PHONY: traces-run-vk

traces-run-con:
	mkdir -p traces/consensus
	npx @informalsystems/quint test --output traces/consensus/{}.itf.json ./tests/consensus/consensusTest.qnt
.PHONY: traces-run-con

traces-run: traces-run-vk traces-run-con
.PHONY: traces-run

traces: traces-sim traces-run
.PHONY: traces
