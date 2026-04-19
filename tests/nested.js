export function get_order() {
  return {
    account: "0xAlice",
    amount: 500n,
    token: {
      symbol: "FOO",
      decimals: null,
      totalSupply: 100n,
    },
  };
}
