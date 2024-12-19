# Malachite source code directory
MALACHITE_CODE_DIR?=$(realpath $(CURDIR)/../../code)

# Number of nodes for the testnet
NODES_COUNT?=5

# Malachite home directory
NODES_HOME?=$(HOME)/.malachite

# Binary name
APP_BINARY?=informalsystems-malachitebft-example-channel

# Is this a RELEASE build?
ifeq ($(strip $(RELEASE)),)
BINARY_PATH="$(MALACHITE_CODE_DIR)/target/debug/$(APP_BINARY)"
else
BINARY_PATH="$(MALACHITE_CODE_DIR)/target/release/$(APP_BINARY)"
RELEASE_FLAG=--release
endif

CC=$(shell which cargo)

all: build testnet start

build:
	@cd $(MALACHITE_CODE_DIR) && $(CC) build $(RELEASE_FLAG) --package $(APP_BINARY)

testnet: build
	@$(BINARY_PATH) testnet --home $(NODES_HOME) --nodes $(NODES_COUNT)

start: testnet
	@for i in $$(seq 1 $(NODES_COUNT)); do \
	  o=$$(($$i - 1)) ;\
	  cd $(NODES_HOME)/$$o ;\
	  nohup $(BINARY_PATH) start --home $(NODES_HOME)/$$o & \
	done

stop:
	@killall -v -u $$UID $(APP_BINARY) 2> /dev/null || true

log:
	@for i in $$(seq 1 $(NODES_COUNT)); do \
	  o=$$(($$i - 1)) ;\
	  ls -lh $(NODES_HOME)/$$o/nohup.out ;\
	done

clean: stop
	@test -n "$(NODES_HOME)" && test "$$(echo "$(NODES_HOME)" | wc -c)" -gt 2
	rm -rf $(NODES_HOME)

restart: clean start

shortrun: restart
	@sleep 6
	$(MAKE) stop
