export default function getDef () {
  return {
    keyword: 'defaults',
    type: 'object',
    schemaType: 'object',
    modifying: true,
    valid: true,
    compile: (defaults) => (data) => {
      for (const [name, val] of Object.entries(defaults)) data[name] ??= val
    },
  }
}
