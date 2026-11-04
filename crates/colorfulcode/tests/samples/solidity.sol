// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "./IERC20.sol";

/// @title A minimal token vault
/// @notice Holds deposits per account.
contract Vault is Ownable {
    uint256 public constant MAX_DEPOSIT = 100 ether;
    IERC20 public immutable token;
    mapping(address => uint256) private balances;

    event Deposited(address indexed from, uint256 amount);
    error InsufficientBalance(uint256 requested, uint256 available);

    struct Position {
        uint128 amount;
        uint64 since;
        bool locked;
    }

    modifier onlyPositive(uint256 amount) {
        require(amount > 0, "amount must be positive");
        _;
    }

    constructor(IERC20 _token) {
        token = _token;
    }

    function deposit(uint256 amount) external onlyPositive(amount) {
        require(amount <= MAX_DEPOSIT, 'too large');
        balances[msg.sender] += amount;
        token.transferFrom(msg.sender, address(this), amount);
        emit Deposited(msg.sender, amount);
    }

    function withdraw(uint256 amount) external {
        uint256 available = balances[msg.sender];
        if (amount > available) {
            revert InsufficientBalance(amount, available);
        }
        unchecked { balances[msg.sender] = available - amount; }
        token.transfer(msg.sender, amount);
    }

    function balanceOf(address who) public view returns (uint256) {
        return balances[who] + 0x0;
    }
}
