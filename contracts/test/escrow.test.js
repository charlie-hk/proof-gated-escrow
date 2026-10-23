const { expect } = require("chai");
const { ethers } = require("hardhat");
const { loadFixture, time } = require("@nomicfoundation/hardhat-network-helpers");

describe("Escrow", function () {
  const jobId = ethers.id("job-1001");
  const commitment = "0x" + "ab".repeat(32);
  const DISPUTE_WINDOW = 3 * 24 * 60 * 60; // 3 days

  async function deploy() {
    const [customer, provider, verifier, outsider] = await ethers.getSigners();
    const Escrow = await ethers.getContractFactory("Escrow");
    const escrow = await Escrow.connect(verifier).deploy(verifier.address);
    return { escrow, customer, provider, verifier, outsider };
  }

  it("records the verifier at construction", async function () {
    const { escrow, verifier } = await loadFixture(deploy);
    expect(await escrow.verifier()).to.equal(verifier.address);
  });

  it("funds a job and holds the value", async function () {
    const { escrow, customer, provider } = await loadFixture(deploy);
    const amount = ethers.parseEther("1");

    await expect(
      escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount })
    )
      .to.emit(escrow, "Funded")
      .withArgs(jobId, customer.address, provider.address, amount);

    expect(await ethers.provider.getBalance(await escrow.getAddress())).to.equal(amount);
    expect(await escrow.stateOf(jobId)).to.equal(1); // Funded
  });

  it("rejects zero-value and zero-provider funding", async function () {
    const { escrow, customer, provider } = await loadFixture(deploy);
    await expect(
      escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: 0 })
    ).to.be.revertedWithCustomError(escrow, "ZeroAmount");
    await expect(
      escrow.connect(customer).fund(jobId, ethers.ZeroAddress, DISPUTE_WINDOW, { value: 1 })
    ).to.be.revertedWithCustomError(escrow, "ZeroProvider");
  });

  it("rejects double funding of the same job id", async function () {
    const { escrow, customer, provider } = await loadFixture(deploy);
    const amount = ethers.parseEther("1");
    await escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount });
    await expect(
      escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount })
    ).to.be.revertedWithCustomError(escrow, "BadState");
  });

  it("lets only the verifier release, paying the provider and storing the commitment", async function () {
    const { escrow, customer, provider, verifier, outsider } = await loadFixture(deploy);
    const amount = ethers.parseEther("2");
    await escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount });

    await expect(
      escrow.connect(outsider).releaseWithVerification(jobId, commitment)
    ).to.be.revertedWithCustomError(escrow, "NotVerifier");

    await expect(
      escrow.connect(verifier).releaseWithVerification(jobId, commitment)
    ).to.changeEtherBalance(provider, amount);

    expect(await escrow.stateOf(jobId)).to.equal(2); // Released
    const job = await escrow.jobs(jobId);
    expect(job.commitment).to.equal(commitment);
  });

  it("cannot release a job twice", async function () {
    const { escrow, customer, provider, verifier } = await loadFixture(deploy);
    const amount = ethers.parseEther("1");
    await escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount });
    await escrow.connect(verifier).releaseWithVerification(jobId, commitment);
    await expect(
      escrow.connect(verifier).releaseWithVerification(jobId, commitment)
    ).to.be.revertedWithCustomError(escrow, "BadState");
  });

  it("refunds the customer only after the dispute window", async function () {
    const { escrow, customer, provider } = await loadFixture(deploy);
    const amount = ethers.parseEther("1");
    await escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount });

    await expect(
      escrow.connect(customer).refundAfterTimeout(jobId)
    ).to.be.revertedWithCustomError(escrow, "DisputeWindowOpen");

    await time.increase(DISPUTE_WINDOW + 1);

    await expect(
      escrow.connect(customer).refundAfterTimeout(jobId)
    ).to.changeEtherBalance(customer, amount);
    expect(await escrow.stateOf(jobId)).to.equal(3); // Refunded
  });

  it("blocks a non-customer from refunding", async function () {
    const { escrow, customer, provider, outsider } = await loadFixture(deploy);
    const amount = ethers.parseEther("1");
    await escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount });
    await time.increase(DISPUTE_WINDOW + 1);
    await expect(
      escrow.connect(outsider).refundAfterTimeout(jobId)
    ).to.be.revertedWithCustomError(escrow, "NotCustomer");
  });

  it("cannot refund a released job", async function () {
    const { escrow, customer, provider, verifier } = await loadFixture(deploy);
    const amount = ethers.parseEther("1");
    await escrow.connect(customer).fund(jobId, provider.address, DISPUTE_WINDOW, { value: amount });
    await escrow.connect(verifier).releaseWithVerification(jobId, commitment);
    await time.increase(DISPUTE_WINDOW + 1);
    await expect(
      escrow.connect(customer).refundAfterTimeout(jobId)
    ).to.be.revertedWithCustomError(escrow, "BadState");
  });
});
