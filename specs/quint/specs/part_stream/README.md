## Quint specification for proposal parts streaming

Check that the main invariant holds at each step (over 5 steps):

```
$ quint verify part_stream.qnt --max-steps 5 --invariant inv 
[ok] No violation found (176881ms).
```

Verify that the process eventually terminates (in at most 5 steps):

```
$ quint verify part_stream.qnt --max-steps 5 --temporal 'eventually(isDone)'
[ok] No violation found (176881ms).
```

Find a counter-example showing that the process terminates (in at most 5 steps):

```
$ quint verify part_stream.qnt --max-steps 5 --invariant 'not(isDone)'
State 4: state invariant 0 violated.                              I@16:16:53.536
Found 1 error(s)                                                  I@16:16:53.543
The outcome is: Error                                             I@16:16:53.576
An example execution:

[State 0] { state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 } }

[State 1]
{
  state:
    {
      buffer: [(1, { msgType: DATA, payload: "Data 1", sequence: 1 })],
      emitted: [],
      finReceived: false,
      initMessage: None,
      initReceived: false,
      nextSequence: 0,
      received: Set({ msgType: DATA, payload: "Data 1", sequence: 1 }),
      totalMessages: 0
    }
}

[State 2]
{
  state:
    {
      buffer: [(1, { msgType: DATA, payload: "Data 1", sequence: 1 }), (2, { msgType: DATA, payload: "Data 2", sequence: 2 })],
      emitted: [],
      finReceived: false,
      initMessage: None,
      initReceived: false,
      nextSequence: 0,
      received: Set({ msgType: DATA, payload: "Data 1", sequence: 1 }, { msgType: DATA, payload: "Data 2", sequence: 2 }),
      totalMessages: 0
    }
}

[State 3]
{
  state:
    {
      buffer:
        [(1, { msgType: DATA, payload: "Data 1", sequence: 1 }), (2, { msgType: DATA, payload: "Data 2", sequence: 2 }), (3, { msgType: FIN, payload: "Fin", sequence: 3 })],
      emitted: [],
      finReceived: true,
      initMessage: None,
      initReceived: false,
      nextSequence: 0,
      received: Set({ msgType: DATA, payload: "Data 1", sequence: 1 }, { msgType: DATA, payload: "Data 2", sequence: 2 }, { msgType: FIN, payload: "Fin", sequence: 3 }),
      totalMessages: 4
    }
}

[State 4]
{
  state:
    {
      buffer: [],
      emitted:
        [
          { msgType: INIT, payload: "Init", sequence: 0 },
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 }
        ],
      finReceived: true,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 4,
      received:
        Set(
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 },
          { msgType: INIT, payload: "Init", sequence: 0 }
        ),
      totalMessages: 4
    }
}

[violation] Found an issue (9558ms).
error: found a counterexample
```

Find a counter-example showing that the process eventually terminates (in at most 5 steps):

```
$ quint verify part_stream.qnt --max-steps 5 --temporal 'not(eventually(isDone))'
State 5: state invariant 0 violated.                              I@16:15:04.901
Found 1 error(s)                                                  I@16:15:04.991
The outcome is: Error                                             I@16:15:05.644
An example execution:

[State 0]
{
  __InLoop: false,
  __q::temporalProps_init: false,
  __saved___temporal_t_1: false,
  __saved___temporal_t_2: false,
  __saved___temporal_t_3: true,
  __saved_state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 },
  __temporal_t_1: false,
  __temporal_t_2: false,
  __temporal_t_3: true,
  __temporal_t_3_unroll: false,
  __temporal_t_3_unroll_prev: false,
  state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 }
}

[State 1]
{
  __InLoop: false,
  __q::temporalProps_init: false,
  __saved___temporal_t_1: false,
  __saved___temporal_t_2: false,
  __saved___temporal_t_3: true,
  __saved_state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 },
  __temporal_t_1: false,
  __temporal_t_2: false,
  __temporal_t_3: true,
  __temporal_t_3_unroll: false,
  __temporal_t_3_unroll_prev: false,
  state:
    {
      buffer: [],
      emitted: [{ msgType: INIT, payload: "Init", sequence: 0 }],
      finReceived: false,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 1,
      received: Set({ msgType: INIT, payload: "Init", sequence: 0 }),
      totalMessages: 0
    }
}

[State 2]
{
  __InLoop: false,
  __q::temporalProps_init: false,
  __saved___temporal_t_1: false,
  __saved___temporal_t_2: false,
  __saved___temporal_t_3: true,
  __saved_state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 },
  __temporal_t_1: false,
  __temporal_t_2: false,
  __temporal_t_3: true,
  __temporal_t_3_unroll: false,
  __temporal_t_3_unroll_prev: false,
  state:
    {
      buffer: [(2, { msgType: DATA, payload: "Data 2", sequence: 2 })],
      emitted: [{ msgType: INIT, payload: "Init", sequence: 0 }],
      finReceived: false,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 1,
      received: Set({ msgType: DATA, payload: "Data 2", sequence: 2 }, { msgType: INIT, payload: "Init", sequence: 0 }),
      totalMessages: 0
    }
}

[State 3]
{
  __InLoop: false,
  __q::temporalProps_init: false,
  __saved___temporal_t_1: false,
  __saved___temporal_t_2: false,
  __saved___temporal_t_3: true,
  __saved_state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 },
  __temporal_t_1: false,
  __temporal_t_2: false,
  __temporal_t_3: true,
  __temporal_t_3_unroll: false,
  __temporal_t_3_unroll_prev: false,
  state:
    {
      buffer: [(2, { msgType: DATA, payload: "Data 2", sequence: 2 }), (3, { msgType: FIN, payload: "Fin", sequence: 3 })],
      emitted: [{ msgType: INIT, payload: "Init", sequence: 0 }],
      finReceived: true,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 1,
      received: Set({ msgType: DATA, payload: "Data 2", sequence: 2 }, { msgType: FIN, payload: "Fin", sequence: 3 }, { msgType: INIT, payload: "Init", sequence: 0 }),
      totalMessages: 4
    }
}

[State 4]
{
  __InLoop: false,
  __q::temporalProps_init: false,
  __saved___temporal_t_1: false,
  __saved___temporal_t_2: false,
  __saved___temporal_t_3: true,
  __saved_state: { buffer: [], emitted: [], finReceived: false, initMessage: None, initReceived: false, nextSequence: 0, received: Set(), totalMessages: 0 },
  __temporal_t_1: false,
  __temporal_t_2: false,
  __temporal_t_3: true,
  __temporal_t_3_unroll: true,
  __temporal_t_3_unroll_prev: false,
  state:
    {
      buffer: [],
      emitted:
        [
          { msgType: INIT, payload: "Init", sequence: 0 },
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 }
        ],
      finReceived: true,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 4,
      received:
        Set(
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 },
          { msgType: INIT, payload: "Init", sequence: 0 }
        ),
      totalMessages: 4
    }
}

[State 5]
{
  __InLoop: true,
  __q::temporalProps_init: false,
  __saved___temporal_t_1: false,
  __saved___temporal_t_2: false,
  __saved___temporal_t_3: true,
  __saved_state:
    {
      buffer: [],
      emitted:
        [
          { msgType: INIT, payload: "Init", sequence: 0 },
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 }
        ],
      finReceived: true,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 4,
      received:
        Set(
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 },
          { msgType: INIT, payload: "Init", sequence: 0 }
        ),
      totalMessages: 4
    },
  __temporal_t_1: false,
  __temporal_t_2: false,
  __temporal_t_3: true,
  __temporal_t_3_unroll: true,
  __temporal_t_3_unroll_prev: true,
  state:
    {
      buffer: [],
      emitted:
        [
          { msgType: INIT, payload: "Init", sequence: 0 },
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 }
        ],
      finReceived: true,
      initMessage: Some({ msgType: INIT, payload: "Init", sequence: 0 }),
      initReceived: true,
      nextSequence: 4,
      received:
        Set(
          { msgType: DATA, payload: "Data 1", sequence: 1 },
          { msgType: DATA, payload: "Data 2", sequence: 2 },
          { msgType: FIN, payload: "Fin", sequence: 3 },
          { msgType: INIT, payload: "Init", sequence: 0 }
        ),
      totalMessages: 4
    }
}

[violation] Found an issue (172272ms).
error: found a counterexample
```
