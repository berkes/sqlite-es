# [Project Name]

[Short introductionary paragraph]

## Component 1
[ Explanation and tooling for Component 1]

## Component 2
[ Explanation and tooling for Component 1]

## Component N
[ Explanation and tooling for Component 1]

## Tools

* `./bin/console` runs an interactive console which allows interaction
    with the blockchain
* `make`, outlined in Quickstart, is used to wrap all the steps to get
    going, run tests, build and deploy the platform.

## Quickstart

Requirements:

* The Firefox, Brave or Chrom{e,uim} browser with
  [metamask](https://metamask.io/) installed.
* A POSIX complient OS: Windows is currently not supported as
  development platform.

### Install

In order to install the platform on development machine, run

    make install

This installs and configures the dependencies.


### Run

After installing the dependencies, on the development machine, run

    make

This builds and runs the platform locally.

### Test

After installing the dependencies, on the development machine, run

    make test

This builds and runs the tests locally.

### Release

After finishing the changes, a release can be prepared with

    make release

This uses amongst others, git-flow to create and tag a new release. It
bumps the version number. Add `VERSION=x.y.z-special` to set a specific
version, instead of incrementing the next minor version (1.2.3 -> 1.3.0)

### Deploy

After running a build, testing successfully, one can deploy with

    make deploy

This checks preconditions such as proper git-tags, branches, permissions
and sanity checks and when met, deploys current release.

