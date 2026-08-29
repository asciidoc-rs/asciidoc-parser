import Ajv from 'ajv/dist/2020.js'
import defaultsKeyword from '../lib/ajv-keyword-defaults.js'
import assert from 'node:assert/strict'
import { fileURLToPath } from 'node:url'
import fs from 'node:fs'
import ospath from 'node:path'
import { before, describe, it } from 'node:test'
import schema from 'asciidoc-asg/schema.js'

describe('asg schema', () => {
  let ajv
  let fixturesDir = ospath.join(ospath.dirname(fileURLToPath(import.meta.url)), 'fixtures')

  before(() => {
    ajv = new Ajv({ allErrors: false, discriminator: true, keywords: [defaultsKeyword()] })
  })

  it('should be valid', () => {
    assert.ok(ajv.validateSchema(schema))
  })

  it('should compile', () => {
    assert.doesNotThrow(() => ajv.compile(schema))
  })

  it('should validate sample data with list', () => {
    const fixturePath = ospath.join(fixturesDir, 'sample-1.json')
    const data = JSON.parse(fs.readFileSync(fixturePath))
    const validate = ajv.getSchema(schema.$id) || ajv.compile(schema)
    assert.ok(validate(data))
  })

  it('should validate sample data with header and paragraph', () => {
    const fixturePath = ospath.join(fixturesDir, 'sample-2.json')
    const data = JSON.parse(fs.readFileSync(fixturePath))
    const validate = ajv.getSchema(schema.$id) || ajv.compile(schema)
    assert.ok(validate(data))
  })

  it('should validate sample data with unknown property', () => {
    const fixturePath = ospath.join(fixturesDir, 'sample-3.json')
    const data = JSON.parse(fs.readFileSync(fixturePath))
    const validate = ajv.getSchema(schema.$id) || ajv.compile(schema)
    assert.ok(!validate(data))
  })
})
