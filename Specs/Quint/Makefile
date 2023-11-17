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

traces:
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPrecommitValue 30
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPolkaAny 30
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPolkaNil 30
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitSkip 30
.PHONY: traces
