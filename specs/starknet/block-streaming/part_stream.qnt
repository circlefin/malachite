// -*- mode: Bluespec; -*-

// TODO:
// Explore case when duplicate messages are sent and write tests:
    // Change isDone logic
        // val emittedFinMessage: bool = {
        //    state.emitted.toSet().exists(msg => msg.msgType == FIN)
        // }


module part_stream {
    import spells.* from "./spells"
    import binary_heap.* from "./binary_heap"

    type Sequence = int
    type Payload = str

    type MessageType = INIT | DATA | FIN

    type Message = {
        sequence: Sequence,
        msgType: MessageType,
        payload: Payload
    }

    pure def createMessage(seq: Sequence, typ: MessageType, payload: Payload): Message =
        { sequence: seq, msgType: typ, payload: payload }

    type State = {
        buffer: BinaryHeap[Message],
        initMessage: Option[Message],
        received: Set[Message],
        nextSequence: Sequence,
        totalMessages: int,
        finReceived: bool,
        emitted: List[Message],
    }

    var state: State
    // Required for MBT
    var incomingMessage: Option[Message]

    val orderedMessages = List(
        createMessage(0, INIT, "Init"),
        createMessage(1, DATA, "Data 1"),
        createMessage(2, DATA, "Data 2"),
        createMessage(3, FIN, "Fin")
    )

    val messages = orderedMessages.toSet()

    action init = all{
        state' = {
            buffer: List(),
            nextSequence: 0,
            totalMessages: 0,
            initMessage: None,
            finReceived: false,
            emitted: List(),
            received: Set(),
        },
        incomingMessage' = None
    }

    action step = any {
        done,
        all{
            messages.exclude(state.received).size() > 0,
            nondet msg = oneOf(messages.exclude(state.received))
            processMessage(msg)
        }
    }

    action done = all {
        require(isDone),
        state' = state,
        incomingMessage' = None
    }

    // There is no check if duplicate message arrived
    action processMessage(message: Message): bool = all{
        incomingMessage' = Some(message),
        match message.msgType {
            | INIT => handleInit(message)
            | FIN => handleFin(message)
            | _ => handleOther(message)
        }
    }

    action handleInit(message: Message): bool = all {
        require(message.msgType == INIT),
        require(message.sequence == 0),

        val state1 = { 
            ...state, 
            initMessage: Some(message), 
            received: state.received.union(Set(message))
        }
        val state2 = emit(state1, message)
        val state3 = emitEligibleMessages(state2)

        state' = state3
    }

    action handleFin(message: Message): bool = all {
        require(message.msgType == FIN),

        val state1 = { 
            ...state, 
            finReceived: true, 
            totalMessages: message.sequence + 1,
            received: state.received.union(Set(message)),
            buffer: state.buffer.insert((message.sequence, message)),
        }
        val state2 = emitEligibleMessages(state1)

        state' = state2
    }

    action handleOther(message: Message): bool = all {
        require(message.msgType != INIT),
        require(message.msgType != FIN),

        val state1 = { 
            ...state, 
            received: state.received.union(Set(message)),
            buffer: state.buffer.insert((message.sequence, message)),
        }
        val state2 = emitEligibleMessages(state1)

        state' = state2
    }

    def emitEligibleMessages(state: State): State = {
        state.buffer.foldl(state, (acc, el) => {
            val message = el._2
            if (message.sequence == acc.nextSequence) {
                emit({ ...acc, buffer: acc.buffer.drop() }, message)
            } else {
                acc
            }
        })
    }

    def emit(state: State, message: Message): State = {
        {
            ...state,
            emitted: state.emitted.append(message),
            nextSequence: state.nextSequence + 1
        }
    }

    val isDone: bool = {
        messages.exclude(state.received) == Set()
    }

    val inv = {
        if (isDone) all {
            state.initMessage != None,
            state.finReceived,
            state.received == messages,
            state.emitted == orderedMessages,
            state.emitted.length() == state.totalMessages,
        } else all {
            state.finReceived implies (state.totalMessages > 0 and state.nextSequence <= state.totalMessages),
            state.received.subseteq(messages),
            state.emitted.toSet().subseteq(messages),
        }
    }


    run MessagesInRightOrderTest = {
        init
        .then(processMessage(createMessage(0, INIT, "Init")))
        .then(processMessage(createMessage(1, DATA, "Data 1")))
        .then(processMessage (createMessage(2, DATA, "Data 2")))
        .then(processMessage(createMessage(3, FIN, "Fin")))
        .expect(state.emitted == List(createMessage(0, INIT, "Init"), createMessage(1, DATA, "Data 1"), createMessage(2, DATA, "Data 2"), createMessage(3, FIN, "Fin")))
    }

    run MessagesInWrongOrderTest = {
        init
        .then(processMessage(createMessage(1, DATA, "Data 1")))
        .then(processMessage(createMessage(0, INIT, "Init")))
        .then(processMessage(createMessage(3, FIN, "Fin")))
        .then(processMessage (createMessage(2, DATA, "Data 2")))
        .expect(state.emitted == List(createMessage(0, INIT, "Init"), createMessage(1, DATA, "Data 1"), createMessage(2, DATA, "Data 2"), createMessage(3, FIN, "Fin")))
    }

    run InitReceivedTest = {
        init
        .then(processMessage(createMessage(0, INIT, "Init")))
        .then(processMessage (createMessage(2, DATA, "Data 2")))
        .expect(state.initMessage == Some(createMessage(0, INIT, "Init")))
    }

    run FinReceivedTest = {
        init
        .then(processMessage(createMessage(3, FIN, "Fin")))
        .then(processMessage(createMessage(1, DATA, "Data 1")))
        .expect(state.finReceived == true)
    }

    temporal eventuallyDone =
        eventually(isDone)
}