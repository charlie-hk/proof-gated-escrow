// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title Proof-Gated Escrow
/// @notice Holds a customer's payment for a job until an off-chain verifier
///         attests that the submitted work meets the job's acceptance criteria.
///         The attestation is bound to a `commitment` hash produced by the Rust
///         verifier over (canonical spec || canonical submission), so release is
///         tied to exactly one verified spec/submission pair.
/// @dev    Deliberately small and dependency-free. The verifier key is the only
///         trusted party for release; disputes fall back to a timeout refund.
contract Escrow {
    enum State {
        None,
        Funded,
        Released,
        Refunded
    }

    struct Job {
        address customer;
        address provider;
        uint256 amount;
        uint64 fundedAt;
        uint64 disputeWindow; // seconds the customer must wait before refund
        bytes32 commitment; // set when released, for auditability
        State state;
    }

    /// @notice The address whose signature (here: direct call) attests verdicts.
    address public immutable verifier;

    mapping(bytes32 => Job) public jobs; // jobId => Job

    event Funded(bytes32 indexed jobId, address indexed customer, address indexed provider, uint256 amount);
    event Released(bytes32 indexed jobId, address indexed provider, uint256 amount, bytes32 commitment);
    event Refunded(bytes32 indexed jobId, address indexed customer, uint256 amount);

    error NotVerifier();
    error BadState();
    error ZeroProvider();
    error ZeroAmount();
    error DisputeWindowOpen();
    error NotCustomer();

    modifier onlyVerifier() {
        if (msg.sender != verifier) revert NotVerifier();
        _;
    }

    constructor(address verifier_) {
        require(verifier_ != address(0), "verifier=0");
        verifier = verifier_;
    }

    /// @notice Customer funds a job, naming the provider and a dispute window.
    /// @param jobId Unique job identifier (keccak of the off-chain job id).
    /// @param provider Who gets paid on a verified release.
    /// @param disputeWindow Seconds before the customer may self-refund.
    function fund(bytes32 jobId, address provider, uint64 disputeWindow) external payable {
        if (provider == address(0)) revert ZeroProvider();
        if (msg.value == 0) revert ZeroAmount();
        if (jobs[jobId].state != State.None) revert BadState();

        jobs[jobId] = Job({
            customer: msg.sender,
            provider: provider,
            amount: msg.value,
            fundedAt: uint64(block.timestamp),
            disputeWindow: disputeWindow,
            commitment: bytes32(0),
            state: State.Funded
        });

        emit Funded(jobId, msg.sender, provider, msg.value);
    }

    /// @notice Verifier releases escrow to the provider against a verified commitment.
    /// @dev The `commitment` is stored so anyone can later match it to the
    ///      verifier's deterministic output for the same inputs.
    function releaseWithVerification(bytes32 jobId, bytes32 commitment) external onlyVerifier {
        Job storage j = jobs[jobId];
        if (j.state != State.Funded) revert BadState();

        j.state = State.Released;
        j.commitment = commitment;

        uint256 amount = j.amount;
        (bool ok, ) = payable(j.provider).call{value: amount}("");
        require(ok, "transfer failed");

        emit Released(jobId, j.provider, amount, commitment);
    }

    /// @notice Customer reclaims funds after the dispute window elapses without
    ///         a verified release (e.g. the provider never delivered).
    function refundAfterTimeout(bytes32 jobId) external {
        Job storage j = jobs[jobId];
        if (j.state != State.Funded) revert BadState();
        if (msg.sender != j.customer) revert NotCustomer();
        if (block.timestamp < j.fundedAt + j.disputeWindow) revert DisputeWindowOpen();

        j.state = State.Refunded;

        uint256 amount = j.amount;
        (bool ok, ) = payable(j.customer).call{value: amount}("");
        require(ok, "refund failed");

        emit Refunded(jobId, j.customer, amount);
    }

    function stateOf(bytes32 jobId) external view returns (State) {
        return jobs[jobId].state;
    }
}
