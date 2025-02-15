# Malachite source code directory
MALACHITE_CODE_DIR?=$(realpath $(CURDIR)/../../code)

# Number of nodes for the testnet
NODES_COUNT?=5

# Malachite home directory
NODES_HOME?=$(HOME)/.malachite

# Binary name
APP_BINARY?=informalsystems-malachitebft-starknet-app

# Is this a RELEASE build?
ifeq ($(strip $(RELEASE)),)
BINARY_PATH=$(MALACHITE_CODE_DIR)/target/debug/$(APP_BINARY)
else
BINARY_PATH=$(MALACHITE_CODE_DIR)/target/release/$(APP_BINARY)
RELEASE_FLAG=--release
endif

CC=cargo

help:
	@echo "Commands:"
	@echo "  make               - show help"
	@echo "  make build         - build binary"
	@echo "  make -B build      - rebuild binary"
	@echo "  make testnet       - build configuration"
	@echo "  make -B testnet    - rebuild configuration"
	@echo "  make start / stop  - start/stop testnet in the background"
	@echo "  make log           - show log files' path"
	@echo "  make clean         - stop testnet and clean up all files"
	@echo "Environment variables:"
	@echo "  APP_BINARY         - the file name to run (informalsystems-malachitebft-starknet-app)"
	@echo "  MALACHITE_CODE_DIR - the source code directory ($(realpath $(CURDIR)/../../code))"
	@echo "  NODES_COUNT        - the number of local nodes to create (5)"
	@echo "  NODES_HOME         - home folder for the node configuration ($$HOME/.malachite)"
	@echo "  RELEASE            - if set, build a release version of the code (<unset> = debug version)"
.PHONY: help

###                       ###
### file creation targets ###
###                       ###

# Build the binary
$(BINARY_PATH):
	@cd $(MALACHITE_CODE_DIR) && $(CC) build $(RELEASE_FLAG) --package $(APP_BINARY)

# Create the configuration
$(NODES_HOME)/0:
	@$(MAKE) $(BINARY_PATH)
	@$(BINARY_PATH) testnet --home $(NODES_HOME) --nodes $(NODES_COUNT)


###                 ###
### manual commands ###
###                 ###


start:
	@$(MAKE) $(NODES_HOME)/0
	@for i in $$(seq 1 $(NODES_COUNT)); do \
	  o=$$(($$i - 1)) ;\
	  cd $(NODES_HOME)/$$o ;\
	  nohup "$(BINARY_PATH)" start --home $(NODES_HOME)/$$o & \
	done
.PHONY: start

stop:
	@killall -v -u $$UID $(APP_BINARY) 2> /dev/null || true
.PHONY: stop

build:
	@$(MAKE) $(BINARY_PATH)
.PHONY: build

testnet:
	@$(MAKE) $(NODES_HOME)/0
.PHONY: testnet

log:
	@for i in $$(seq 1 $(NODES_COUNT)); do \
	  o=$$(($$i - 1)) ;\
	  ls -lh $(NODES_HOME)/$$o/nohup.out 2> /dev/null || echo $(NODES_HOME)/$$o/nohup.out ;\
	done

clean:
	@$(MAKE) stop
	@test -n "$(NODES_HOME)" && test "$$(echo "$(NODES_HOME)" | wc -c)" -gt 2
	rm -rf $(NODES_HOME)

