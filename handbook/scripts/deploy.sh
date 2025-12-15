#!/bin/bash

mdbook build

git clone ssh://git@github.com/open-rmf/crossflow-handbook temp-deploy-checkout --branch main --single-branch --depth 1
cd temp-deploy-checkout
git checkout --orphan gh-pages
git reset

cp -r ../book/* .

git add .
git commit -am "Publish to GitHub Pages"
git push origin gh-pages --force
