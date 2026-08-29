import fs from 'node:fs'

export default JSON.parse(fs.readFileSync('./schema.json'))
