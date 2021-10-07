clean:
	rm -rf test-certs

setup:
	mkdir -p test-certs


generate-certs:	setup generate-ca-crt generate-server-crt generate-client-crt generate-pk12-certs

# run generate-certs first
generate-pk12-certs:	generate-server-pk12 generate-client-pk12

generate-ca-key:	
	openssl genrsa  -out test-certs/ca.key 4096

# this is CA pem file
generate-ca-crt:	generate-ca-key
	openssl req -x509 -new -nodes -key test-certs/ca.key -out test-certs/ca.crt \
		-subj /C=US/ST=CA/L=Sunnyvale/O=Fluvio/OU=Eng/CN=fluvio.io



generate-server-key:	
	openssl genrsa -out test-certs/server.key 4096


generate-server-csr:	generate-server-key
	openssl req -new -key test-certs/server.key \
		-out test-certs/server.csr \
		-config  cert.conf 


# generate anonymous pk12
.PHONY: generate-server-pk12
generate-server-pk12:	
	openssl pkcs12 -export -out test-certs/server.pfx -inkey test-certs/server.key -in test-certs/server.crt -certfile  test-certs/ca.crt -passout pass:test



verify-csr:
	openssl req -in test-certs/server.csr -noout -text

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
	openssl pkcs12 -export -out test-certs/client.pfx -inkey test-certs/client.key -in test-certs/client.crt -certfile  test-certs/ca.crt -passout pass:test


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

stop-nginx:
	nginx -s quit

