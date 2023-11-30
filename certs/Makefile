# -legacy is an openssl3 option for pkcs12 formats whose default
# has shifted
PFX_OPTS ?= "-legacy"

clean:
	rm -rf test-certs

setup:
	mkdir -p test-certs
	openssl version

generate-certs:	setup generate-ca-crt generate-intermediate-ca-crt generate-server-crt generate-intermediate-chain generate-client-crt generate-pk12-certs

# run generate-certs first
generate-pk12-certs:	generate-server-pk12 generate-client-pk12

### CA Generation

generate-ca-key:
	openssl genrsa  -out test-certs/ca.key 4096

generate-ca-crt:	generate-ca-key
	openssl req -x509 -new -nodes -key test-certs/ca.key -out test-certs/ca.crt \
		-subj /C=US/ST=CA/L=Sunnyvale/O=Fluvio/OU=Eng/CN=fluvio.io

### Intermediate CA Generation

generate-intermediate-ca-key:
	openssl genrsa  -out test-certs/intermediate-ca.key 4096

generate-intermediate-ca-csr: generate-intermediate-ca-key
	openssl req -new \
    -key test-certs/intermediate-ca.key \
    -out test-certs/intermediate-ca.csr \
    -subj /C=US/ST=CA/L=Sunnyvale/O=Fluvio/OU=Eng/CN=intermediate.fluvio.io \
    -config intermediate-cert.conf

generate-intermediate-ca-crt: generate-intermediate-ca-csr
	openssl x509 -req \
    -in test-certs/intermediate-ca.csr \
    -out test-certs/intermediate-ca.crt \
		-CA test-certs/ca.crt \
		-CAkey test-certs/ca.key \
		-CAcreateserial  \
		-days 500 \
		-extensions v3_inter \
		-extfile openssl.cnf

### Non-Intermediate Chain Server

generate-server-key:
	openssl genrsa -out test-certs/server.key 4096
	cp test-certs/server.key test-certs/server-hs.key

generate-server-csr:	generate-server-key
	openssl req -new -key test-certs/server.key \
		-out test-certs/server.csr \
		-config  cert.conf


### Intermediate Chain Server

generate-intermediate-server-key:
	openssl genrsa -out test-certs/intermediate-server.key 4096


generate-intermediate-server-csr:	generate-intermediate-server-key
	openssl req -new -key test-certs/intermediate-server.key \
		-out test-certs/intermediate-server.csr \
		-config  cert.conf

# generate anonymous pk12
.PHONY: generate-server-pk12
generate-server-pk12:
	openssl pkcs12 -export -out test-certs/server.pfx ${PFX_OPTS} -inkey test-certs/server.key -in test-certs/server.crt -certfile  test-certs/ca.crt -passout pass:test



verify-csr:
	openssl req -in test-certs/server.csr -noout -text

### Non-Intermediate Chain

decrypt-server-crt:
	openssl x509 -in test-certs/server.crt   -noout -text

generate-server-crt:	generate-server-csr
	openssl x509 -req \
		-in test-certs/server.csr \
		-out test-certs/server.crt \
		-CA test-certs/ca.crt \
		-CAkey test-certs/ca.key \
		-CAcreateserial  \
		-days 500 \
		-extensions v3_end \
		-extfile openssl.cnf

### Intermediate Chain

decrypt-intermediate-server-crt:
	openssl x509 -in test-certs/intermediate-server.crt   -noout -text

generate-intermediate-server-crt:	generate-intermediate-server-csr
	openssl x509 -req \
		-in test-certs/intermediate-server.csr \
		-out test-certs/intermediate-server.crt \
		-CA test-certs/intermediate-ca.crt \
		-CAkey test-certs/intermediate-ca.key \
		-CAcreateserial  \
		-days 500 \
		-extensions v3_end \
		-extfile openssl.cnf

generate-intermediate-chain: generate-intermediate-ca-crt generate-intermediate-server-crt
	cat test-certs/ca.crt test-certs/intermediate-ca.crt test-certs/intermediate-server.crt > test-certs/intermediate-full.crt

#################################
#
#  Client Certificates
#

generate-client-key:
	openssl genrsa -out test-certs/client.key 4096

generate-client-csr:	generate-client-key
	openssl req -new -key test-certs/client.key -out test-certs/client.csr \
		-subj "/C=US/ST=CA/O=MyOrg, Inc./CN=client.com"

generate-client-crt:	generate-client-csr
	openssl x509 -req \
		-days 365 -in test-certs/client.csr \
		-out test-certs/client.crt \
		-CA test-certs/ca.crt -CAkey test-certs/ca.key -CAcreateserial \
		-extensions v3_end \
		-extfile openssl.cnf

generate-client-pk12:
	openssl pkcs12 -export -out test-certs/client.pfx ${PFX_OPTS} -inkey test-certs/client.key -in test-certs/client.crt -certfile  test-certs/ca.crt -passout pass:test


# for non mac
test-curl:
	curl -v -s -k --key client.key --cert client.crt "https://127.0.0.1:8443/hello/world"

install-curl-ssl:
	 brew upgrade curl-openssl

test-mac-curl:
	 /usr/local/opt/curl-openssl/bin/curl -v -k -s --key certs/client.key --cert certs/client.crt "https://127.0.0.1:8443/hello/world"


MAKE_DIR = $(dir $(realpath $(firstword $(MAKEFILE_LIST))))

start-nginx:
	nginx -c $(MAKE_DIR)/nginx.conf

start-intermediate-nginx:
	nginx -c $(MAKE_DIR)/intermediate-nginx.conf

stop-nginx:
	nginx -s quit

