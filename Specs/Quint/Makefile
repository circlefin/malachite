parse:
	npx @informalsystems/quint parse asyncModelsTest.qnt
	npx @informalsystems/quint parse consensusTest.qnt
	npx @informalsystems/quint parse executor.qnt
	npx @informalsystems/quint parse statemachineTest.qnt
	npx @informalsystems/quint parse voteBookkeeperTest.qnt
.PHONY: parse

con-test:
	npx @informalsystems/quint run consensusTest.qnt
.PHONY: con-test

sm-test:
	npx @informalsystems/quint test statemachineTest.qnt
.PHONY: sm-test

vk-test:
	npx @informalsystems/quint test voteBookkeeperTest.qnt
.PHONY: vk-test

test: con-test sm-test vk-test
.PHONY: test

traces:
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPrecommitValue
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPolkaAny
	./scripts/gen-traces.sh voteBookkeeperTest.qnt emitPolkaNil
.PHONY: traces
