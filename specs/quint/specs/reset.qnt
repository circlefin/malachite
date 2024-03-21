module reset {

type Address = str

type Validator = {
    address : Address,
    power: int
}

type Registration = {
    epoch: int,
    index: int,
    address: Address,
    update: int
    // we might add content later
}

type Proof = 
    | LTWOheight(int)
    | Nil


type L1Block = {
    height : int,
    registrations: Set[Registration],
    proof: Proof
}

type L2Block = {
    height: int,
    registrations: Set[Registration],
    valset: Set[Validator],
    nextValset: Set[Validator],
    pendingRegistrations: Set[Registration]
}

var L1: List[L1Block]

var L2: List[L2Block]


action init = all {
    L1' = List(),
    L2' = List()
}


}