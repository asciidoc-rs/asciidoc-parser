#!/usr/bin/env node

import Ajv from 'ajv/dist/2020.js'
import defaultsKeyword from '../lib/ajv-keyword-defaults.js'
import fs from 'node:fs'
import process from 'node:process'
import { parseArgs } from 'node:util'

const options = {
  out: { type: 'boolean', desc: 'output the validated and compiled data' },
  'validate-schema': { type: 'boolean', desc: 'validate the schema' },
}
const { positionals, values } = parseArgs({ args: process.argv.slice(2), options, allowPositionals: true, strict: true })
const ajv = new Ajv({ allErrors: false, discriminator: true, keywords: [defaultsKeyword()] })
const schema = JSON.parse(fs.readFileSync('schema.json'))
if (values['validate-schema']) ajv.validateSchema(schema)
const validate = ajv.compile(schema)
const inputFile = positionals[0] || 'sample-1.json'
let data = JSON.parse(fs.readFileSync(inputFile))
if (Array.isArray(data)) {
  const inlines = data
  data = {
    name: 'document',
    type: 'block',
    blocks: [{ name: 'paragraph', type: 'block', inlines }]
  }
  if (inlines.length) {
    const location = [inlines[0].location[0], inlines[inlines.length - 1].location[1]]
    data.location = location
    data.blocks[0].location = location
  }
}
if (!validate(data)) {
  console.log(inputFile)
  console.dir(validate.errors[0], { depth: Infinity })
}
if (values.out) console.dir(data, { depth: Infinity })
