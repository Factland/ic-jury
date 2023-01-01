all:
	cargo build

test:
	(cd tests;node tests.js)
